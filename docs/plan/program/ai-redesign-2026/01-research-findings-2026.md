# AI Redesign 2026 — Research Findings (2026-06 SOTA)

> 來源：背景研究 workflow `wf_18f54e54-e57`（39 個獨立 web-research subagent，8 個領域，各走 _landscape → deep-dive → adversarial critic_）。
> 每個領域的完整原始結論（候選清單、deep dive、critic）保存在 [`research-appendix/`](research-appendix/)。
> 本文是 distilled 結論層；具體選型與理由在 [02-architecture-decisions.md](02-architecture-decisions.md)。
> 注意：研究中點名的具體 model id 僅供理解 landscape，**不得寫進產品文案**（見 [00](00-scope-use-cases-constraints.md) §3.5）。

---

## 0. 三個貫穿全局的結論（先讀這個）

研究最重要的輸出不是「選哪個庫」，而是三個會重塑整個架構的事實，8 個領域的 critic 反覆獨立指出：

### 0.1 真正的瓶頸是「embedding 生成」，不是 vector index

對 1440 萬條做一次性 embedding，用 Qwen3-Embedding-0.6B（decoder-style、last-token pooling、~600M 參數）在 4 核 / 無 GPU 上：

- 唯一可查到的 CPU 單點是 ~30ms/doc，**但那是 i9-13900HK（16 核混合）**，不是目標機。
- 估到 4 核 3GHz：~50–120ms/short doc（int8）→ **14.4M 一次跑完約 60 小時 ~ 4 天的純 CPU 時間**。
- 結論：**首次建索引必須是可暫停、可續跑、限流、off-thread 的背景 job**，永遠不是 import 的阻塞前置；建索引期間（可能數天）UI 用 FTS5/bm25 + deterministic intelligence 正常服務。這正是既有 lease-based job queue 的頭號用例。
- 衍生的大決策：**雙層 embedding 模型策略**（見 §2.3）—— 用 pure-Rust static embedding（model2vec-rs，~8000 docs/sec）先把 14.4M 在「數十分鐘」內鋪一層 baseline，再用 Qwen3-0.6B 做背景品質升級 pass。

### 0.2 8GB × 14.4M：向量必須 quantize + disk-resident + rerank + MRL；in-memory fp32 HNSW 不可能

記憶體數學（所有 critic 一致）：

- 14.4M × 1024-dim **f32 = ~59 GB**（純資料）；in-memory HNSW（含 graph）≈ 55–61 GB → **是 8GB 的 7–8 倍，物理上不可能**。
- 連 int8 × 1024d ≈ 14.7 GB 也**塞不進 8GB RAM**。
- 唯一可行形狀：**量化 codes 常駐 RAM + 全精度/int8 向量在磁碟、只在 rerank 時讀 + MRL 維度截斷 + disk-resident / IVF 佈局**。
  - binary（1-bit）× 1024d = ~1.84 GB；MRL-512 binary ≈ 0.9 GB；MRL-256 ≈ 0.45 GB → 常駐 RAM 可接受。
  - RaBitQ / int8 + 從 mmap sidecar rerank top-K，recall 可回到 ~0.95–0.98。
- 另一個漂亮的結論：**~100 萬向量以下，根本不用 ANN**——直接 int8 SIMD flat scan（1M×1024B ≈ 1GB，精確、零建構、零 corruption 風險）。多數使用者好幾年都在這個區間。

### 0.3 in-app 推理：embedding 可行，chat 邊際

- **Embedding**：Qwen3-0.6B Q8 ≈ 400MB 常駐、20–100ms/chunk → 便宜到可以 in-app 預設。
- **Chat / LLM**：8GB 上一個 4B Q4 ≈ 2.5GB weights + 1–2GB KV，疊上 Chromium WebView（0.3–0.7GB）+ SQLCipher cache + 進行中的 import working set → 直接到 swap-thrash 懸崖；4 核還會和 import 搶 CPU。CPU-only ~15–20 tok/s，做短摘要勉強、做長 agent loop 痛苦。
- 結論：**chat 預設走 external（使用者已在跑的 Ollama / LM Studio / OpenAI-compatible / cloud）**；若要 in-app chat，用 **llama-server sidecar**（不是 in-process），預設關、consent-gated、與 import 互斥、限 0.6B–4B Q4。

---

## 1. LLM 接入（取代 2023 litellm router）

**2026 的做法是「兩層」，不是一個胖 router：**

- **Layer 1 — transport**：speak OpenAI-compatible `/v1/chat/completions`（+漸增的 `/v1/responses`）作為 local / OpenAI-shaped 的 floor；**Anthropic / Gemini 走各自 native adapter**（不要走它們的 OpenAI-compat shim，否則 strict structured output 與 prompt caching 失效——critic 用 Anthropic 官方文件證實 compat 層是「not at feature parity」）。
- **Layer 2 — agent/tools**：**MCP（Model Context Protocol）已成跨廠標準**（OpenAI / Google / Ollama / LM Studio 原生支援）。用 official **rmcp** crate 把 PathKeep 的 history/evidence 工具暴露成 in-process MCP server。

**候選 verdict：**
| 候選 | 角色 | verdict |
| --- | --- | --- |
| **rust-genai** (806★) | 多 provider 純 Rust transport，零 native dep，model id 是 runtime string | 最佳「feature-per-line」，但 <6k★、單一維護者 → **需風險評估 + pin + vendor**，藏在自有 trait 後 |
| **async-openai** (1.9k★) | OpenAI / Responses dialect adapter | 作為 OpenAI-compat adapter 可用；非全棧 |
| **rig** (7.7k★) | 全功能 Rust agent framework | **唯一憑星數過門檻**，但 churn 大、夾帶我們不要的 vector-store/agent machinery → 當參考，不當 load-bearing dep |
| **rmcp** (official, ~3.5k★) | MCP SDK | 憑「reputable org」過門檻；**in-app 工具用 plain Rust trait，rmcp 只用在對外 MCP 邊界** |
| **mistral.rs / llama.cpp** | in-app/local 推理 | 見 §0.3，sidecar-first |
| **OpenRouter / Responses API** | optional cloud preset | 只作可選 adapter，不作預設 routing 層（雙重 egress 邊界） |

**取向**：PathKeep 自有 `LlmProvider` trait（chat / stream / tool-call / structured-output / capabilities），底下先接 genai（風評估後）或手寫 3–4 個 provider JSON；API key 用 `secrecy::SecretString`（drop 即清零、log redact、永不序列化進 trace）。structured output 用 provider native schema mode，但**永遠保留 serde validate-and-repair fallback**（constrained = 可解析，不等於語義正確）。
_來源：appendix `_compact_0`、`_ALL_CRITICS` §llm-integration。_

---

## 2. Embedding —— in-app 推理（Rust / CPU，預設路徑）

### 2.1 引擎

| 引擎                                                | native-dep / 供應鏈                                                                                                             | verdict                                                                                                                                |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| **candle** (HF, 20.5k★, MIT/Apache)                 | 純 Rust CPU kernels，**零下載 C++、零 cmake**                                                                                   | **供應鏈最乾淨**，過門檻無需風評估；但 CPU 比 tuned ONNX 慢 ~5–8x                                                                      |
| **fastembed-rs**（935★，含 `qwen3` candle backend） | qwen3 path 只編 candle；但 `ort` 是硬依賴（即使不用也編進來，且 default features 會**下載 ONNX binary = 禁用模式**）            | 用 `default-features=false, features=["qwen3"]`；<6k★需風評估；或乾脆 vendor 它那 ~1k 行 qwen3.rs                                      |
| **ort (ONNX Runtime)**（2.3k★）                     | default `download-binaries` = 禁用；static-link 要自己 vendor 一份 ONNX Runtime build（無現成 crate 完整 vendored）= ADR 級承諾 | **預設 path 拒絕**；只作 power-user 優化 path                                                                                          |
| **llama.cpp (llama-cpp-2, 590★)**                   | build.rs **vendored C++ submodule、cargo build 時編譯** = 偏好模式                                                              | 可行；最大優點是**embedding 與 reranking 共用同一引擎**（見 §7）；GGUF Qwen3 有 community 轉檔 bug（~20% recall 掉 / all-zeros）需驗證 |
| **mistral.rs** (7.3k★)                              | candle-based                                                                                                                    | 純做 embedding 過重，除非 chat 也用它                                                                                                  |

### 2.2 預設模型

Qwen3-Embedding-0.6B（**Apache-2.0**，1024-dim、MRL 可截 32–1024、100+ 語言、last-token pooling、需自行 L2-normalize、query 要 instruction prefix）—— 符合使用者指定的預設。EmbeddingGemma-300M 技術上更快（encoder、768-dim、~22ms）但 **Gemma 自訂授權有 use-restriction flow-down**，只作 user-selectable 備選，不作 bundled 預設。

### 2.3 critic 的關鍵增補：雙層模型 + binary-quant index

- **FAST tier**：`model2vec-rs` + 一個 static multilingual 模型（純 Rust、token-vector lookup、**~8000 docs/sec 單執行緒**、模型 8–30MB、零 native dep）。把 14.4M 首鋪從「數天」變「數十分鐘」。
- **QUALITY tier**：Qwen3-0.6B int8，背景、限流、可續跑的升級 pass，建好原子換入。
- **INDEX**：MRL 截斷（256/512）後 **binary quant（1-bit/dim）** 常駐 + int8/f32 rerule rescore → fit 8GB。
- 校正：`uint8 ONNX + MRL-256` **不是免費組合**——常見 uint8 ONNX 匯出本身不支援 MRL；MRL 必須在 fp32/bf16 hidden state 上做，再量化。

### 2.4 模型下載 / 完整性

`hf-hub` 純 Rust 但**不驗 hash**；PathKeep 必須自己 pin SHA-256、支援完全離線首跑（bundle 或 sideload 預設模型）、下載 off-thread + 分片 + 可取消 + 進度。
_來源：appendix `_compact_1`、`_ALL_CRITICS` §embedding-inapp。_

---

## 3. Embedding —— external providers

- **一個 OpenAI-compatible `/v1/embeddings` adapter** 覆蓋 Ollama（native `/api/embed` + `/v1`）、LM Studio、vLLM、llama-server 與多數 cloud；只給非 OpenAI-shaped 的 cloud（Gemini 的 `task_type`/單輸入、Voyage/Cohere 的 `input_type`+`output_dtype`）寫小 adapter。建議**自寫薄 reqwest+serde client**，不加 async-openai（<6k★且該呼叫很簡單）。
- **三個必須抽象掉的 provider 差異**：每請求 batch 上限（OpenAI 2048/300K tok、Voyage 1000/320K tok、Gemini 單輸入）、是否需要 query/document instruction 不對稱、輸出是否已 normalize。
- **三個正確性陷阱**（critic 強調）：
  1. `dimensions` 參數在 local compat facade 上常被**靜默忽略**（Ollama issues #235/#5154）→ **永遠讀實際回傳向量長度**，別信 config。
  2. normalization 不一致（OpenAI 已 normalize、Ollama L2、raw Qwen3 沒有）→ **一律防禦性 L2-normalize**，尤其 MRL 截斷後。
  3. query vs document 角色不對稱會掉 recall → 角色旗標必須穿過 trait 傳到每次呼叫。
- **換模型 = 整個 index 失效**（換 model / dim / normalization / instruction template 任一）。共識 pattern：把 embedding 設定存成 **versioned fingerprint** `{provider, model_id, effective_dim, output_dtype, normalized, instruction_template, version}`，runtime 探測填入，蓋章在 sidecar；mismatch → 明確警告 + 可續跑 re-embed + 遷移期 dual-index 查詢。PathKeep 的 canonical SQLCipher 本就是 source text，所以**全量 re-embed 永遠可行，sidecar 正確地可丟棄**。
- 雲端 provider 的 in-API 量化（Voyage/Cohere int8/binary）可省本地量化步驟，但只適用雲端、且必 consent-gate。
  _來源：appendix `_compact_2`、`_ALL_CRITICS` §embedding-external。_

---

## 4. Vector store（使用者點名「影響很大」）

**所有路線都收斂到同一個形狀**（§0.2）：disk-resident + 量化 codes 常駐 + 從磁碟 rerule rescore + MRL，**外加 ~1M 以下走 flat int8 scan 不用 ANN**。分歧只在「用哪個引擎承載」。

| 方案                                                   | verdict                                                     | 重點                                                                                                                                                                                                                                                 |
| ------------------------------------------------------ | ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **LanceDB / Lance**（6.7k★，Apache-2.0，primary-Rust） | novel-area critic 的**首選**（pending on-target benchmark） | disk-first IVF_PQ + on-disk 全精度 rescore；天然「可重建 sidecar」；clustered IVF 在 4 核冷快取下比 mmap-HNSW 少很多隨機 SSD 讀。**build-time 需 protoc/C toolchain/openssl/pkg-config** → 要解決（vendor/feature）才符合供應鏈規則                  |
| **usearch**（4.2k★，Apache-2.0，unum）                 | **conditional / recommend with 風評估**                     | crate **vendor C++、cargo build 編譯** = 偏好模式；b1 binary @14.4M×1024 ≈1.84GB；`view()` mmap 把 index 留磁碟。**警告**：14.4M 下 mmap-HNSW 隨機 graph hop 會踩 SSD page-fault（~20,000ns）→ 只在 in-RAM binary 模式可行，disk-view 在此規模是陷阱 |
| **hand-rolled pure-Rust IVF + RaBitQ + disk-rerank**   | memory-area critic 的**首選**                               | RaBitQ（SIGMOD 2025，1-bit + 校正標量、無 codebook 訓練、有誤差界）是**純算術、零 native dep**；IVF 在 4 核分鐘級建好（HNSW 要小時級）；最乾淨供應鏈，但最多自有代碼                                                                                 |
| **hnsw_rs**（242★，pure Rust）                         | plan-B                                                      | 供應鏈滿分（零 C/C++），但 graph 常駐（M=16 @14.4M ≈2.76GB）、**無量化、無 delete/update**；要自己疊量化 + disk rerank                                                                                                                               |
| **LEANN**（12.4k★，recompute 範式）                    | **拒絕**（互動路徑）                                        | 存圖、查詢時即時重算 ~25–30 個 embedding：省 disk（~97%），但延遲是 GPU 數字（2.5–7.1s on RTX 4090）；4 核 CPU 會是**數秒到數十秒/query** + Python/FAISS/Boost/protobuf 經 Homebrew = 禁用模式。只可作「冷檔深召回」的可選 mode                      |
| **sqlite-vec**                                         | **拒絕**                                                    | stable 只有 brute-force（14.4M 秒級）、ANN 還是 alpha；且是 loadable extension + 向量進 SQLite，違反既定立場（最多放獨立 sidecar .db，無增益）                                                                                                       |
| **Turbovec / turbopuffer / Qdrant / Milvus-lite**      | 拒絕                                                        | Turbovec 3.5k★/單人/3 週新（借 TurboQuant idea，別依賴）；turbopuffer 雲端閉源；Qdrant/Milvus 是 server process                                                                                                                                      |

**兩個 critic 的最終分歧 = 真正要你定的 fork（見 §9）**：memory-area 推「純 Rust IVF+RaBitQ」，novel-area 推「LanceDB」，usearch 居中。三者都需在**真實 4 核/8GB/消費級 SSD** 上 benchmark 才能下最終結論。
_來源：appendix `_compact_3`、`_compact_4`、`_ALL_CRITICS` §vector-store-\*。_

---

## 5. AI agent —— loop / durability / context

2026 共識：**agent loop 本身就是個 thin while-loop（呼叫模型 → 有 tool call 就執行並回填 → 重複）**，工程全在外圍。對 Rust desktop 是好消息。

- **框架抉擇**：**roll-your-own loop**。重 Python/TS SDK（Claude Agent SDK / OpenAI Agents SDK / LangGraph）都是 Python/TS、且 durability 靠外部 server（Temporal/Restate）→ 不適合 local-first desktop，**只當 pattern 參考**。Rust 原生（rig 7.7k★ 可選作 provider/tool plumbing，但不給 durability/compaction/subagent/memory）。
- **durability**：**擴展既有 lease-based job queue + 一個 sidecar SQLite**（agent trace 是可重建衍生狀態，**不進 canonical 加密 DB**）。鐵律：每個 model/tool 輸出**先 journal 再 observe**；resume 時**replay journaled bytes，絕不重呼模型**（避免重複收費與發散）；每個 tool call 有 idempotency key；PME 審批用 long-pause 釋放 worker。**不要引入** Temporal/Restate/DBOS（都要 server/外部進程）。
- **context engineering**：以 **recency pruning（整對丟棄 stale tool-result）+ 輕量 summarize** 為主，昂貴的 LLM auto-compact 為最後手段；working tokens 壓在 ~32k 以下（Context Rot）；**帶 row id + count，evidence just-in-time 取，絕不 inline 大結果集**；在 sidecar 存一張 **citation table（row id → evidence）讓引用熬過 compaction**（直接服務透明度要求）。
- **sub-agents**：支援但**並發上限 ~2**、低優先、餵 pre-filtered 結果、禁止觸發 bulk embedding（否則破壞 4 核 UI 流暢硬指標）。
- **memory**：實作輕量 Anthropic memory-tool **pattern**（不是重型 memory 平台），存 sidecar、consent-gated、可檢視/可刪、recall 重用既有 embedding 棧。
- **MCP**：in-app 工具用 plain Rust trait（低開銷、緊 capability 控制）；rmcp 只在**對外**邊界。
  _來源：appendix `_compact_5`、`_ALL_CRITICS` §agent-architecture。_

---

## 6. Agent tools / skills / code execution

**三層、預設安全、漸進解鎖**（critic 的整合結論）：

- **Layer 0 — 檢索地基（先做、永遠在、AI-optional-safe）**：~8–12 個 bounded、read-only 結構化工具，把使用者要的顆粒度做成顯式工具（raw visits / sessions / query-groups / threads / domains / daily rollups / insights），各回 bounded top-K 並帶 `visit_id` provenance，外加 `fetch_visits(ids)` 驗證工具。**FTS5+bm25+trigram 是不可協商的那一個**（已在 3.50 棧、即時、AI 關閉時也在）。
- **Layer 1 — 語義/hybrid（additive、consent-gated）**：見 §4 / §7。
- **Layer 2 — code-mode（opt-in、capability-gated 升級，不是預設）**：2026 的 **Programmatic Tool Calling / "code mode"**——agent 寫一段 JS，在沙箱裡 fan-out 多個 BM25/vector/hybrid 查詢、本地 join/filter，只回蒸餾後的 top-K（Anthropic PTC 報 +11% 準確率 / −24% input tokens；MCP-as-code 150k→2k tokens）。**直接命中**使用者「一個 turn 跑多個檢索 + 寫代碼輔助」的需求。
  - **沙箱**：**Wasmtime**（18.2k★，Apache-2.0，Bytecode Alliance，**純 Rust、零 C build**，capability-based 預設零權限、ResourceLimiter 記憶體上限、epoch/fuel CPU 上限 → 過門檻無需風評估）+ **Javy（JS→WASM guest）**：ship 一個預編譯 `.wasm`，agent 生成的 JS 只是餵進去的資料，**不直接依賴 rquickjs**（880★）也不 ship V8（deno_core 太重）。guest **只能呼叫一組 read-only query host fn，無 DB handle、無 SQL string、無 fs/net**。
  - 小模型（0.6B 預設 tier）**不開 code-mode**（寫不出可靠腳本）→ 雙路徑：capable model 用 code-mode，弱模型用 classic single-tool-call。
- **Skills（SKILL.md / agentskills.io 開放標準）**：**用在對外 MCP 面**（讓 Claude Code/Cursor 等外部 agent 透過 progressive disclosure 學會「顆粒度階梯 + BM25-vs-vector-vs-hybrid + 引用 visit id」）；PathKeep 自家小模型**烘一個緊湊 compiled prompt 即可**，不跑多檔 skill 協議（在 0.6B 上是浪費 budget）。skills 與 MCP tools **互補不互斥**（2026 共識）。
- **「奇怪 properties」進階搜尋**：最乾淨是 **read-only、參數化、跑在 derived/intelligence sidecar 上的受限 Text-to-SQL**（`PRAGMA query_only`/immutable，絕不碰 canonical SQLCipher）。
  _來源：appendix `_compact_6`、`_ALL_CRITICS` §agent-tools-skills-codeexec。_

---

## 7. Hybrid search + reranking

- **融合**：**RRF（k=60）為預設**（rank-only，無需 normalize，天然解掉 BM25 無界 vs cosine [-1,1] 的尺度衝突，14.4M 下只作用在兩個 top-K 候選列、近乎零成本）；**convex/weighted 為可選進階**（有標註資料時上限更高，但個人歷史無 per-user 標註 → RRF 的 zero-shot 穩健性正好）。
- **rerank**：**預設關、opt-in 精排**。只在候選大而雜（N≫K，~10:1）且第一階段 MRR@5 < ~0.85 時才有增益，否則純延遲成本。CPU 可行的小 cross-encoder：Qwen3-Reranker-0.6B 的 **seq-cls 版**（單次 forward，與 embedding 同家族；**native causal yes/no 版 ~380–400ms autoregressive、別誤測**）或 bge-reranker-v2-m3（~140–200ms）。**lazy-load、keep-alive 後釋放**，絕不與 embedder/chat 同時常駐。
- **引擎復用**：若 in-app embedding 用 llama.cpp，rerank 用 `pooling=rank` **共用同一 native dep**（零新供應鏈面）。**警告**：community Qwen3-Reranker GGUF 常壞（缺 `cls.output.weight` → ~4.5e-23 亂分）→ 必須用官方 `convert_hf_to_gguf.py` 轉 + build-time 分數健全性斷言。
- **ColBERT / late-interaction**：**拒絕**（multi-vector index 爆儲存：8.8M passage = 142 GiB；對 desktop 是 overkill，「好第一階段 + 一次 cross-encoder」已拿下多數品質）。
- **FTS5 scaling 陷阱**：`ORDER BY rank LIMIT` 在大表會爆（6M row 案例 20s+）→ external-content FTS5 + 緊 LIMIT + 熱路徑避免 vtable join 大表。
- **agent 工具**：`search_bm25` / `search_vector` / `search_hybrid` 三個分開的工具，讓 agent 自行升級（bm25 → hybrid → +rerank），且 `search_bm25` 在無 embedding 時也能用。
  _來源：appendix `_compact_7`、`_ALL_CRITICS` §hybrid-search-rerank。_

---

## 8. 跨領域：供應鏈 / native-dep 裁定速查

| 元件                                       | 星數 / 維護     | native build                               | 裁定                                            |
| ------------------------------------------ | --------------- | ------------------------------------------ | ----------------------------------------------- |
| candle                                     | 20.5k, HF       | 純 Rust                                    | ✅ 過門檻                                       |
| Wasmtime                                   | 18.2k, BA       | 純 Rust                                    | ✅ 過門檻                                       |
| rmcp                                       | ~3.5k, official | 純 Rust                                    | ✅ reputable-org 例外                           |
| rig                                        | 7.7k            | 純 Rust                                    | ✅ 過門檻（但 churn）                           |
| mistral.rs                                 | 7.3k            | candle                                     | ✅ 過門檻                                       |
| lance/LanceDB                              | 6.7k            | **build-time protoc/C/openssl/pkg-config** | ⚠️ 過星門檻，但 build 鏈要先解決才合規          |
| usearch                                    | 4.2k, unum      | **vendored C++ in crate**（偏好模式）      | ⚠️ 需風險評估                                   |
| Extism                                     | 5.2k            | wasm host                                  | ⚠️ 邊緣，非必要                                 |
| fastembed-rs                               | 935             | candle(qwen3)/ort                          | ⚠️ 需風險評估 + pin                             |
| genai                                      | 806, 單人       | 純 Rust                                    | ⚠️ 需風險評估 + pin + vendor                    |
| llama-cpp-2                                | 590             | vendored C++ submodule                     | ⚠️ 需風評估（信任實落在 upstream llama.cpp）    |
| ort                                        | 2.3k            | **default 下載 binary = 禁用**             | ❌ 預設拒絕（除非自 vendor ONNX build，ADR 級） |
| hnsw_rs                                    | 242             | 純 Rust                                    | ⚠️ 需風評估（plan-B）                           |
| 純 Rust RaBitQ crates                      | ≤17, 有 ARM bug | 純算術                                     | ❌ 不依賴；**自寫小型 vendored 實作**           |
| LEANN / sqlite-vec / faiss-next / Turbovec | 各見 §4         | 多違規                                     | ❌                                              |

通則：能用 vendored-C++-in-crate（rusqlite 那類）或純 Rust 就行；`secrecy::SecretString` 存所有 API key；任何外部 MCP server / cloud provider 都是 egress 邊界，**硬預設關、逐個 consent**。

---

## 9. decision forks —— 已於 2026-06-20 與使用者鎖定

| Fork                  | 決定                                                                                                                                                                                                                                               |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| in-app LLM(chat) 範圍 | **純 external，不打包任何生成 LLM**（含 sidecar）；in-app 推理只做 embedding                                                                                                                                                                       |
| embedding 預設策略    | **單一模型路徑**（預設 Qwen3-0.6B），不加 model2vec fast tier；但**零模型假設**——dim/pooling/normalization/instruction 全 runtime 偵測，模型隨時可換                                                                                               |
| code-execution 沙箱   | **Wasmtime + Javy(JS)**，opt-in、capability-gated、只讀查詢 host API                                                                                                                                                                               |
| vector 引擎           | **Turbovec**（quantized flat-scan，純 Rust，TurboQuant；維護者經使用者背書）藏在 `VectorIndex` trait 後；usearch/LanceDB 為 fallback。LEANN/Turbovec 排除理由與 Turbovec 復活見對談紀錄；Turbovec 實測特性見 [02](02-architecture-decisions.md) §D |

完整理由、tradeoff、邊界、benchmark gate → [02-architecture-decisions.md](02-architecture-decisions.md)。
