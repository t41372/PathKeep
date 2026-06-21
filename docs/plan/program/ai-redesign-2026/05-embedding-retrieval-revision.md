# AI Redesign 2026 — Embedding & Retrieval Revision (layered tiers + dedup + GPU)

> 狀態：2026-06-21 與使用者鎖定。**修訂 02 §C/§D + D3**（使用者明確同意：「我們確實可以考慮基於 model2vec 的 fast tier 了」+ 提供分層策略評估）。這是 trade-off / 決策修訂文檔，承 [04-current-state-and-execution.md](04-current-state-and-execution.md)。
>
> 動機：W-AI-4b S1 benchmark 證實——candle 0.10.2 CPU 量化路徑無 native int8 kernel（每次 matmul dequant），Q8 把 RAM 砍半（3.4→1.59 GB，終於進 8GB）但慢 ~5×（6.29→1.25 docs/sec）。**任何 in-app candle 精度都無法在 4 核/8GB/無保證 GPU 上於合理時間完成 14.4M 首鋪**。需要分層。

---

## 0. 修訂的鎖定決策

| #                | 原決策                                                                               | 修訂                                                                                                                                                                                                                       |
| ---------------- | ------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **D3**           | 單一模型（Qwen3-0.6B），不加 model2vec fast tier                                     | **加 model2vec/static 基底層**（100% 覆蓋的快速底座）；Qwen3 降為「有界品質層」。使用者 2026-06-21 同意。                                                                                                                  |
| **§C tier 策略** | external + candle 雙 `EmbeddingProvider`，單層                                       | **三層**：static base（全量）→ Qwen3 quality（有界工作集）→ rerank（top-k）。                                                                                                                                              |
| **§D 向量存儲**  | Turbovec quantized flat-scan（2-bit），in-RAM                                        | **binary 召回（RAM）+ int8 rescore（mmap）**；規模過閾值再上 HNSW/IVF。Turbovec 的 2-bit 量化掃描可擔 binary 召回角色；int8 rescore + index 演進為新增。引擎選型（Turbovec vs usearch/sqlite-vec）於 W-AI-5 benchmark 定。 |
| **向量 keying**  | `(history_id, provider, model, content_hash)`（W-AI-4a `.pkvec` 以 history_id 為鍵） | **以 content-hash 去重**：unique 向量按「規範化 URL + 標題 + enrichment 內容哈希」為鍵，visit 事件指向同一向量。14.4M visit → 1–3M unique 向量。**最大且近免費的槓桿**。需 refactor W-AI-4a 的 keying。                    |

---

## 1. 去重（最大槓桿，先做）

瀏覽歷史高度重複（gmail 訪問 5000 次 = 5000 visit、但只需 **1** embedding）。向量以 `content_hash = hash(canonical_url + title + enrichment_summary)` 為鍵；`ai_embeddings` 已是 `UNIQUE(history_id, provider, model, content_hash)`——改為 unique-vector 表（content_hash 主鍵）+ visit→content 映射。預算因此寬鬆一個數量級。所有規模數字仍按 14.4M 上限算（最壞：每條唯一）。

## 2. Tier 0 — static base（全量 100%，任何 CPU）

- **model2vec / static multilingual**（候選 `potion-multilingual-128M`，256-dim，101 語言、從 bge-m3 蒸餾、無上下文長度限制）。CPU 數千–數萬條/秒 → 14.4M ≈ 分鐘–數十分鐘。確定性、可打包、無 GPU 依賴、換模型重嵌也只是分鐘級。
- 本質 = token→向量查表 + (加權) mean pooling，O(序列長)。質量誠實：~MTEB 51 級（比 MiniLM 低 ~8%），無上下文（分不清 bank=銀行/河岸）——但對「標題+URL+短摘要」短文本 + 偏關鍵字查詢，弱點被稀釋；rerank（§5）在結果頂端補回。
- 供應鏈：`model2vec-rs`（MinishLab 官方 Rust 推理，**採用前驗證 stars/維護/license + crates.io**）或自實作（table lookup + mean pool，極小）。模型經 hf-hub + SHA-256 下載，consent-gated。
- 這是 60 年都付得起、任何機器都在線的底座。

## 3. 存儲 — binary 召回 → int8 rescore

256-dim 向量存兩份：

- **binary**（每條 32 B；14.4M ≈ **460 MB** 常駐 RAM）→ SIMD popcount/Hamming 做第一階段召回。
- **int8**（每條 256 B；14.4M ≈ **3.7 GB** mmap 落盤）→ 只對 top 數百/數千候選做高保真 rescore。
- fp32 全量（59 GB @1024 / 14.7 GB @256）**不存**（W-AI-4a 的 .pkvec f32 已從 export 排除；改存 int8/binary，或保留 f32 僅作有界工作集的 rescore 源）。
- 存儲成本只由「維度 × 量化精度」決定，與模型無關 → static 與 Qwen3 都輸出 256-dim 時占用相同。

## 4. Tier 1 — Qwen3-0.6B quality（有界工作集，opt-in）

- candle Qwen3-Embedding-0.6B（W-AI-4b 已交付，parity 0.9995）。**不對全量跑**，只跑有界工作集：**starred ∪ 近 12–24 月 ∪ tagged/noted ∪ 高頻 refind/habit**（帕累托：一小撮 URL 承擔多數查詢）。MRL 截 256/512 + int8。可恢復、可節流、低優先背景；預設關，偵測 GPU/強 CPU 才自動開。工作集有界 → 存儲與時間有界。
- 選擇器：`ai/indexing.rs` 的候選排序加 priority score，**starred 為最高權重**（declared signal）；其次 recency / tag / refind。

## 5. Rerank（結果頂端補質量）

查詢路徑：static binary 全量召回 → int8 rescore → 對最終展示 top-k（~50）過 cross-encoder / Qwen3-Reranker（candle，lazy-load）。每查詢只重排 ~50 條 → 4 核 CPU 輕鬆，卻在「使用者真正看到的結果」補回大模型質量，**無需用大模型 embed 全語料**。W-AI-6。

## 6. Index 演進（避免全量重建）

flat（binary 常駐 RAM）即時零訓練；過閾值（~1–2M）再上 HNSW 或 IVF，建在 binary/int8 上壓內存（HNSW M=16 @14.4M 圖鏈 ~0.9GB；IVF 分布漂移需週期重訓 centroid）。Rust 棧：Turbovec（已驗證）/ usearch / sqlite-vec 候選——W-AI-5 benchmark(S2) 定。每向量打 model id + version + dim + quant + tier。

## 7. GPU 路徑（使用者明確要的選項）

candle 有 Metal / CUDA backend（我們 CPU-only 是為可重現 build + 無保證 GPU 信封）。

- **Metal（Apple Silicon）**：candle bundle Metal kernels（`candle-metal-kernels`）→ 可作 **opt-in cargo feature**，macOS 上相對乾淨；M 系 GPU 上 0.6B 快數量級。
- **CUDA（NVIDIA）**：需 CUDA toolkit（系統依賴）→ ADR / power-user 路徑，可能獨立 build，非預設 binary。
- **使用者選項**：有 GPU 且知成本者可選 **全量重嵌**；或 fast-tier + 重點站點（starred / 近期 / tagged）跑重嵌。全 opt-in、顯示成本估算（PME：「重嵌 N 列，估 ~X 時、~Y GB」）。

## 8. 選擇性重嵌（working set 控制）

heavy 工作集 = **starred + recent(可配 12–24 月) + tagged/noted + 高頻 refind**，使用者可配（toggle 各維度 + 自訂窗口）。starring（見 starring 設計）是最高信度成員。GPU 在場時可一鍵升級到「全量重嵌」。

## 9. 修訂後工作序（承 04 §3；W-AI-4 external+candle 已交付）

1. **Starring MVP**（feature，AI-independent）：`star` 表（canonical_url/domain/query_family）+ 命令 + 全處 toggle + Starred hub + `is:starred` facet + `S` 快捷 + export + i18n×3。提供 heavy 工作集的 declared signal。
2. **W-AI-4c：model2vec static base tier + content-hash 去重**：static `EmbeddingProvider`（`AnyEmbeddingProvider::Static`）+ unique-vector 去重 schema（visit→content）+ 全量快速首鋪。
3. **W-AI-5（修訂）：向量存儲（binary 召回 + int8 rescore）+ semantic search + S2**：引擎選型 benchmark；index 演進。
4. **W-AI-6：hybrid + rerank**（+ starred boost）。
5. **heavy 工作集選擇器 + GPU opt-in（Metal）+ 全量/選擇性重嵌**（可併入 4c/5 或獨立 + 設定 UI 在 W-AI-9）。
6. **W-AI-7 agent → W-AI-8 code-mode → W-AI-9 MCP/skills/consent UX/i18n/hardening**（含 W-AI-4b carryover：download single-flight、KV-reset env-test、degrade doc）。

---

## 10. 開放項

- model2vec-rs 供應鏈驗證（或自實作）；static 模型選型（potion-multilingual-128M vs 其他）。
- binary 召回的 recall 是否足夠（vs int8/HNSW）——S2 量測。
- starred 檢索 boost 權重（過大 → 語義搜尋變書籤列表；需 bounded + 可調，benchmark 驗證）。
- f32 是否保留作 rescore 源（vs 只存 int8）——存儲 vs 質量 trade-off，S2 定。
