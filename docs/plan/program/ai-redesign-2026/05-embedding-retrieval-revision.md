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

查詢路徑：static binary 全量召回 → int8 rescore → 對最終展示 top-k（~50）過 cross-encoder / Qwen3-Reranker（candle，lazy-load）。每查詢只重排 ~50 條 → 4 核 CPU 輕鬆，卻在「使用者真正看到的結果」補回大模型質量，**無需用大模型 embed 全語料**。

> **修訂（W-AI-6，2026-06-21）—neural cross-encoder rerank DEFERRED 到 GPU/heavy tier（reconciliation，非 silent drop）**：W-AI-4b 的 **S1 benchmark** 已證 candle 0.10.2 CPU 量化路徑無 native int8 kernel——0.6B multilingual reranker 在 4 核/8GB baseline ≈ **40s/query**，遠超互動門檻；唯一夠快的 cross-encoder 是 English-only（過 CJK 即廢）。同時 **W-AI-5 的 int8 rescore 已給「召回池的 cosine-accurate 排序」**（recall@1-of-source = 1.0），故 neural rerank 是「結果頂端的增量打磨」，不是核心戰力。據此與 §7（heavy models → GPU opt-in）對齊：neural rerank **降為 GPU/heavy-tier opt-in**，未來 reranker 在場時只重排展示 top-k。**W-AI-6 改交付 MODEL-FREE 品質層**——hybrid **RRF 融合** + **有界 starred boost**（見 §9.4 / §10）：確定性、零模型、在有界召回池上運算（14.4M 不全掃），是這個 baseline 上 honest 的核心品質 win。

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
4. **W-AI-6：hybrid（RRF 融合）+ 有界 starred boost + facet/starred allowlist**（neural rerank DEFERRED 到 GPU tier，見 §5 修訂）。**已交付（2026-06-21）**。
5. **heavy 工作集選擇器 + GPU opt-in（Metal）+ 全量/選擇性重嵌**（可併入 4c/5 或獨立 + 設定 UI 在 W-AI-9）。
6. **W-AI-7 agent → W-AI-8 code-mode → W-AI-9 MCP/skills/consent UX/i18n/hardening**（含 W-AI-4b carryover：download single-flight、KV-reset env-test、degrade doc）。

---

## 10. 開放項

- ~~model2vec-rs 供應鏈驗證（或自實作）；static 模型選型~~ **已定（W-AI-4c，2026-06-21）**：model2vec-rs crates.io license=「Non-standard」（踩 deny SPDX allowlist）+ ~193 star（< 6k gate）→ **自實作**（table-lookup + mean-pool + 選用 L2-norm，PCA/zipf/SIF 蒸餾時已烘進矩陣、inference 不重做），復用 in-tree tokenizers/safetensors/hf-hub、零新依賴。模型選 `minishlab/potion-multilingual-128M`（256-dim、101 語言、bge-m3 蒸餾）。
- ~~binary 召回的 recall 是否足夠（vs int8/HNSW）~~ **S2 已量並用 `RECALL_FLOOR` 重調（W-AI-5，2026-06-21 修訂；真機 64GB、synthetic clustered 256-dim、seeded、k=10）**：兩階段（binary Hamming 召回 → int8 rescore）的 **recall@1-of-source = 1.0000 在 1M/5M/14.4M 全規模**——對「找到查詢近似的那份文件」（使用者真正要的）binary 召回完全足夠。對「精確重現 exact-cosine top-10 排序」（密集 cluster 內近乎相同的鄰居），原本 k'=8×（=64）太淺，recall@10-vs-exact ≈ 0.30–0.34。**X-1 修正：加 `RECALL_FLOOR=2000`**（k' = `max(k×expansion, 2000)`，capped at n，no-facet 與 faceted 兩支都套用，順手消掉「no-facet 池比 faceted 還淺」的反轉）。binary sweep 是 O(n) 與 k' 無關、int8 rescore 數千條是 sub-ms，**故加深 k' 幾乎零延遲成本**。重調後（k'=2000）：recall@10-vs-exact 子樣本（200k）≈ **0.79–0.80**（1M/5M/14.4M 穩定）；**full-n probe**（在真實 n 暴力 exact top-10、20 查詢）= 0.68 @1M、0.50 @5M、0.555 @14.4M。pool-depth sweep（@14.4M）：k'=500→0.58、k'=1000→0.69、k'=5000→0.90、k'=10000→0.96、k'=50000→1.00。**誠實框定：這個 recall@10-vs-exact 偏低，部分是 synthetic 近重複 artifact**（同 cluster 內近乎相同的鄰居，exact 排序對它們本就近乎任意）——它量的是「逼近 exact 排序」，不是 retrieval 品質；retrieval 品質由 recall@1-of-source = 1.0 證明。`RECALL_FLOOR` 是 **robustness/tuning win at ~zero latency，不是 "3× quality" fix**。「逼近 exact 排序」仍交 W-AI-6 的 rerank 在 top-k 補回——這正是分層設計的意圖。HNSW/IVF **暫不需要**（見 engine pick）。
- **查詢路徑 config-drift guard（D1，W-AI-5 修訂）**：`semantic_matches` 在 search 前驗證 live embedding config 是否仍配得上已建的 planes，**garbage score 不再進 merge**：(a) **dim 變更**（user-mutable `provider.config.dimensions` / MRL 截斷）→ 查詢 binarize 成不同 byte 寬度、會 prefix-only 比對——直接比 `query_vector.len() != index.dim()`，命中即推誠實 note（「vector dimension changed … run Build index」）並退 lexical-only、無 semantic hits；(b) **同 dim fingerprint drift**（pooling/normalization/instruction/dtype 變）→ 用 selected engine descriptor（按 build 路徑 `vector_store_for_chunk` 同樣 stamp：真實 dtype/pooling/instruction、provider config id/model、observed query dim）建 live `EmbeddingFingerprint`，呼叫先前的 dead code `planes_are_stale`，stale 即推「rebuild the semantic index」note + 退 lexical-only。另把 `hamming_distance`/`dot_product` 加 `debug_assert_eq!(len)`，未來 caller 再 prefix-compare 會在 debug 立即 trip（release 仍以 `min` 保持 total）。
- ~~starred 檢索 boost 權重（過大 → 語義搜尋變書籤列表；需 bounded + 可調，benchmark 驗證）~~ **已定（W-AI-6，2026-06-21）**：實作 **MODEL-FREE 品質層**，neural rerank DEFERRED 到 GPU/heavy tier（見 §5 修訂；S1 證 0.6B reranker ≈ 40s/query CPU、快的 cross-encoder English-only；int8 rescore 已給 cosine-accurate 排序）。三件落地：
  - **Hybrid RRF 融合**取代 W-AI-5 的 max-merge：每結果 score = `Σ_list weight_list / (rrf_k + rank)`（0-based rank，lexical + semantic 兩串各算）。兩串都命中 → 加總 + `match_reason = "Lexical + semantic match"`；單串 → `"Lexical match"` / `"Semantic match"`。確定性、零模型、只在有界召回池運算（top-k lexical ∪ top-k' semantic，不全掃 corpus）。預設等權（`lexical_weight = semantic_weight = 1.0`、`hybrid_rrf_k = 60`），可調（`AiSettings`，load 時 clamp）。
  - **有界 starred boost**：融合後把分數正規化到 `[0, 1]`，對 starred 頁加 **CAPPED additive delta**（`starred_boost` 預設 **0.15**，clamp `[0, MAX_STARRED_BOOST=0.5]`）。因 normalized 頂是 `1.0` 而 boost cap 是 `0.5`，**一個無關 starred 頁永遠跳不過一個強相關 unstarred 頁**——回歸測試 `starred_boost_promotes_a_relevant_favorite_without_dominating` 證此 bounded property（相關 favorite 被提升、無關 favorite 不主導）。boosted 結果 `match_reason` 帶 `"(Starred)"` 後綴供 FE 顯示（沿用既有 backend-label 慣例，非新 i18n string）。`starred_boost = 0` 或無 star → no-op。
  - **`is:starred` facet 收緊 BOTH plane**：`AiSearchRequest.starred_only` 開啟時，lexical 用 starred post-filter、semantic 用 **`FlatVectorIndex::search` 的 content_key allowlist**（W-AI-5 留的接點）。**content_key 映射**：content_key = `hash(canonical_url + title + enrichment)`，**無法由 URL 單獨重算**（title/enrichment 會變），故走「starred URL/domain → archive `visits.id`（star-driven 有界 seek，`stars::starred_history_ids`，排除 reverted）→ `.pkmap` content_keys（`VisitContentMap::content_keys_for_history_ids`）」，以 `.pkmap` 為 source of truth、非 re-hash。空 allowlist 誠實回 0 semantic hits（沒 starred 頁被索引），非無視 facet。
    - **starred-visit 解析有界（HIGH 修正，2026-06-21；index-seek 落實 Cluster 2a / H-2，2026-06-22）**：`starred_history_ids` 不再「掃 `visits ⋈ urls` 全表、逐列 `matcher.is_starred(url)`」（O(total visits)，14.4M 互動路徑違反 §3）。改 **FORWARD seek**：① URL star → `urls.id`（復用 `enrich_url_star` 的 `idx_urls_url` exact seek + **explicit byte-range `url >= :prefix AND url < :prefix_upper` 前綴範圍 SEEK**，Rust 再 canonicalize 確認）；② domain star → `urls.id`（**INDEX SEEK 於 migration 015 新增的持久化 `urls.registrable_domain` 欄：`WHERE registrable_domain = :domain`，跑 `idx_urls_registrable_domain`**；該欄存 `registrable_domain_for_url(url)`，與 `StarredMatcher::is_starred` 精確等價、無需 Rust re-check）；③ `urls.id` → `visits.id`（chunked `url_id IN (...)`，跑 `idx_visits_visible_url_time`、`reverted_at IS NULL`、群組 ~500 守 SQLite 變數上限）。**H-2 修正**：原本前綴與 domain 兩 pass 用 `LIKE`，但本 DB 預設 `case_sensitive_like = OFF`，BINARY `idx_urls_url` 無法用於 LIKE range，`EXPLAIN QUERY PLAN` 實為全 `SCAN urls`（每 star 一次，違反 14.4M envelope）；改 byte-range（URL）＋持久化欄 seek（domain）後，三 pass 全為 `SEARCH`。全程被 tiny star set 有界（`EXPLAIN QUERY PLAN` 回歸測試守 SEARCH 非 SCAN）。
    - **lexical plane recall（MEDIUM 修正，2026-06-21）**：facet 開時 lexical 池 EXPANDS（`limit × 8`，clamp `[1, 1000]`，比照 semantic `recall_k`），再 post-filter starred + take，**不再**只取「最新 `limit` 條 text match」後過濾（會漏掉視覺較舊但 match query 的 starred 頁；AI-off / no-provider 用戶 lexical 是唯一召回路）。**RESIDUAL（已記、暫收）**：比最新 `limit × 8` 條更舊的 starred match 仍可能漏——把 `url_id IN (...)` 推進 `list_history`（凍結的 multi-path FTS/regex/SQL contract）過於侵入，expanded pool 覆蓋現實情形（一把 starred 頁落在寬鬆 recency 窗內）並誠實降級而非硬截斷到最新 `limit`。
    - ~~**`.pkmap` forward lookup carryover（W-AI-7+）**：`content_keys_for_history_ids` 仍是每 starred-facet query 一趟 O(n) 順序掃整個 `.pkmap`~~ **已關閉（M-11 / XA-PERF-4，2026-06-23 Cluster 2b）**：加 **keyed 排序 sidecar** —— `.pkrev`（按 `content_key` 排序的 `(content_key, history_id)`）與 `.pkfwd`（按 `history_id` 排序的 `(history_id, content_key)`），與 planes 一起從 `.pkmap` 投影（同一個 `.pkvec` fingerprint hash 戳記、staleness 一致、export-excluded、derived），查詢時**按位置 BINARY SEARCH 落盤**（mirror int8 plane 的 seek posture，**~0 額外常駐**）。**兩個方向都變有界**：(a) **always-on 語義 hydration**（result `content_key → visits`，過去 `hydrate_semantic_hits` **每次查詢**都全掃 `.pkmap`、約 2× 真實互動延遲且驅逐 page cache——M-11 的核心缺陷）現 **O(k'·log n)** seek（`.pkrev` binary-search 到 content_key 的連續 run 起點再前掃）；(b) **`is:starred` forward**（`history_id → content_key`）現 **O(starred·log n)** seek（`.pkfwd`）。**read-path guard**：sidecar 缺/stale（舊 index 或 torn pair）即退**權威 `.pkmap` 全掃**（仍正確、只是舊的 O(n) 成本），下次 build 再重投影 sidecar（不在查詢執行緒觸發重建，守 Principle 3）。正確性：sidecar 是 `.pkmap` last-writer-wins map 的同一 `(content_key, history_id)` multiset，故 keyed lookup 回傳**與全掃完全相同**的 history_ids（多訪問頁 fan-out 全保留、無漏無重，hydration 仍挑 most-recent visible visit）。為 `is:starred` 與 always-on hydration 各建一個排序 sidecar 而非單一檔，是因兩方向需不同排序鍵；落盤+二分而非常駐 HashMap 是因 per-visit `.pkmap` 不隨去重縮小（14.4M visit），常駐 reverse map ~300–460 MB 會疊在 binary plane 879 MB 上吃掉 8GB 信封並破壞「binary-only resident」posture。
  - **knobs（`AiSettings`，serde camelCase、load 時 clamp、預設保守）**：`hybrid_rrf_k`(60、`>=1`)、`lexical_weight`/`semantic_weight`(1.0、`[0, 100]`、NaN→default)、`starred_boost`(0.15、`[0, 0.5]`、NaN→default)。FE type 已加（parity），**settings-UI 綁定留 W-AI-9 consent-UX carryover**。
  - **AI-off / no provider** → lexical-only（RRF 退化成單串 = lexical 順序）；空/stale index → 誠實 note（W-AI-5 D1 guard 不動）。`VectorIndex` allowlist post-filter 即 starred-facet 的接點，現已接上。
- ~~f32 是否保留作 rescore 源（vs 只存 int8）~~ **S2 已定（保留 f32 `.pkvec`，但查詢時不用它）**：int8 rescore 的 recall 已等同 binary 召回的覆蓋（rescore 不丟召回給的任何候選——量到 final recall == binary coverage），**故查詢路徑不需 f32 exact tier**。但 f32 `.pkvec` 仍**保留為無損 rebuild SOURCE**（binary/int8 plane 是純投影，fingerprint 變即重投影、零訓練；刪 f32 就得重嵌全量）。
  - **記憶體模型（C1/C2 修訂後，@14.4M/256-dim S2 實測）— code/comments/doc/benchmark 描述同一個模型**：
    - **binary plane = 唯一常駐**：**878.9 MB 常駐**（含 Rust `(u64, Vec<u8>)` 結構開銷的「真實」footprint = 64 B/vec：32 B struct + 32 B packed bits；先前 doc 寫的 439.5 MB 只算 packed bytes、低估了 resident，E1 已修正）。≤ 8GB 信封成立。
    - **int8 plane = 真正落盤、按位置 seek**：**3.68 GB on-disk，NOT resident**（C1）——`FlatVectorIndex` 不再 `read_all` 整盤進 RAM；rescore 對每個召回候選用 `Int8Plane::reader().record_at(position)` 按 `header_len + position × stride` seek 一筆（k' 數千次隨機讀，遠便宜於 O(n) binary sweep，且暖 OS page cache）。**binary position i ↔ int8 position i ↔ 同 content_key** 由 lockstep 投影保證（兩盤同一串流、同序各寫一筆）。先前「mmap 落盤」措辭已改為「binary-resident + int8 seek-on-disk」。
    - **f32 source 14.7 GB**（rebuild-only、排除 export）。
    - **build peak RAM（C2 streaming 投影）≈ 330 MB @14.4M**（dedup key-set + 兩個 write buffer + 一筆 f32 record；**f32 source 從不整盤常駐**）。先前 `read_all` 投影把整個 f32 SET（~14.7GB）+ 完整 binary Vec + 完整 int8 Vec 同時常駐（~19GB peak），破 8GB 信封——現用 `VectorStore::read_records_streaming` 兩趟 stride（pass 1 記 last-writer offset、pass 2 按序 emit），把每筆 f32 直接投進兩個 plane writer。
- **engine pick（S2 已定，2026-06-21；C1/C2/X-1 後重量測）**：**HAND-ROLL flat 兩階段，不加 Turbovec/usearch/sqlite-vec**——binary plane 常駐 **878.9 MB**（真實 footprint 含 Rust 開銷，≤ 8GB）、build（in-RAM）836K vec/s（14.4M ≈ 17.2s）、plane streaming 投影 ≈ 25.8s @14.4M（build peak ≈ 330 MB）。**latency（兩階段、k'=2000、binary sweep + int8 seek rescore）@14.4M：p50 = 105ms / p95 = 130ms**（1M=6.9ms、5M=31ms，O(n) 線性主導；int8 seek 數千筆是 sub-ms，加深 k' 到 2000 仍遠在互動 0.5s 內——比舊 k'=64 的 431ms 還低，因舊量測把 int8 整盤常駐）。**M-11 修正（2026-06-23）**：上述 `index.search(...)` p50/p95 **只量召回+rescore，過去未含 hydration 的 `.pkmap` 第二趟全掃**——當時 hydration 仍是 O(n) `.pkmap` stride，真實互動延遲約 2× 並驅逐 page cache。換成 keyed `.pkrev` sidecar（O(k'·log n) seek）後，**S2 bench 現額外量「search + keyed hydration seek」的 FULL interactive latency**（見 `s2_vector_index_bench.rs` 的 `FULL interactive latency` 行）：keyed hydration 只加數 µs 的 binary-search seek，故 full-path ≈ search-only（不再是 2×）。**建議**：14.4M 尾端若要更低延遲，未來上 HNSW/IVF（建在 binary/int8 上，05 §6）；但**現在不加 dep**（去重後實際 unique 向量 1–3M，1M latency 6.9ms 完全互動；且 W-AI-6 rerank 只重排 top-k 不加召回成本）。`VectorIndex` trait 已是抽象接點，indexed backend 屆時 drop-in 換、非重寫。S2 benchmark（`tests/s2_vector_index_bench.rs`，`PATHKEEP_S2_BENCH=1` gated、seeded PRNG 確定性；現報「真實 resident binary（含 struct 開銷）/ int8 on-disk / streaming build peak / 子樣本 recall + 警告 + full-n probe」）保留作回歸/再決策資料。
- **incremental plane append（C4，DEFER → W-AI-6/later）**：目前 incremental（含 enrichment 變更）pass 是**重投影全部 planes**（O(total) CPU），不是只投影新 content_key。C2 streaming 投影後此舉**已被 RAM bound**（build peak ≪ 1 GB at any scale，源是串流、從不整盤常駐），且跑在 worker 非 UI 執行緒，故對現實 1–3M post-dedup 語料**可接受先 defer**。待語料規模需要時再加 incremental plane-APPEND（只投影新鍵、append 到既有 planes）。接點 `// TODO(W-AI-6/later)` 已留在 `ai/indexing.rs` 重投影處。
- **dedup keying 細節（W-AI-4c 已落地）**：`content_key:u64` = content_hash 前 8 bytes，是 **STORAGE-boundary key**（`.pkvec`/`.pkmap` 的固定寬度佈局）。embed loop 的 **work-dedup 改 key by 完整 `content_hash`**（`indexing::select_embed_targets`，MEDIUM-4 修正）——故兩個 distinct page 即使 u64 碰撞（≈ n²/2^65 ≈ 2.8e-6 @14.4M）也各自 embed，第二頁不會被靜默丟到第一頁的 vector 上；`.pkvec` 層該碰撞存兩筆同 u64（`read_all` last-writer-wins 處理），SQLite `ai_embeddings` 仍存完整 content_hash 作精確身份。`.pkvec` key by content_key（格式不變），`.pkmap`（history_id→content_key）作 visit fan-out。incremental（非 resume）也載入既有 store/map 的 dedup state（MEDIUM-5），故已嵌頁的新訪問只 map、不重嵌、不追加重複 `.pkvec` record。`#fragment` 變體不 collapse（`normalize_visit_url` 不剝 fragment）——刻意，可能是不同頁段。
- **static engine parity（W-AI-4c 已驗）**：hand-rolled static engine vs Python model2vec 的 cosine parity 由 **committed reference fixture**（`tests/fixtures/static_parity_potion_multilingual.json`，12 條含 CJK/emoji/URL/percent-encoded/真 OOV `[UNK]` 輸入）作 STANDING gate 證明，全部 cosine = 1.000000（`PATHKEEP_STATIC_PARITY=1`），不再依賴 test-time Python，也不是 self-check。**unk 處理鐵律**：potion 的 Unigram tokenizer 無 string `unk_token`（unk 是 `[UNK]` id 1，僅以 `unk_id` index 宣告），model2vec 對此 **POOL `[UNK]`**（只對宣告 string `unk_token` 的 BPE/WordPiece 才 drop unk）；engine 精確 mirror——丟 Unigram `[UNK]` 會讓 OOV 行 parity 跌到 ~0.80（gate 抓得到）。`STATIC_MAX_INPUT_TOKENS=2048` 比 model2vec `encode` 預設 `max_length=512` 大，僅作 DoS 上限；fixture 輸入皆 <512 token 故 parity 精確。
