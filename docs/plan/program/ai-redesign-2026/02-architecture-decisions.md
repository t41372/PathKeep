# AI Redesign 2026 — Architecture Decisions

> 狀態：核心 decision forks 已於 2026-06-20 與使用者鎖定；benchmark-gated 項目已標明。
> 研究背書：[01-research-findings-2026.md](01-research-findings-2026.md) + [research-appendix/](research-appendix/)。
> 約束基線：[00-scope-use-cases-constraints.md](00-scope-use-cases-constraints.md)。
> 既有 AI 代碼（rig.rs / LanceDB / ai_sidecar）僅供參考、不約束本設計。

---

## 0. Decision log（已鎖定）

| #   | Fork                    | 決定                                                                                                                       | 理由摘要                                                                                      |
| --- | ----------------------- | -------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| D1  | in-app LLM(chat) 範圍   | **純 external，不打包任何生成 LLM**（含 sidecar）                                                                          | 8GB 上 chat in-app 邊際；in-app 推理只做 embedding。chat 一律走 user-configured provider。    |
| D2  | in-app embedding 引擎   | **candle**（純 Rust）為唯一 in-app 推理引擎，兼跑 embedding 與（可選）reranker                                             | 供應鏈最乾淨、過信任門檻；chat 既走 external，不需要 llama.cpp。                              |
| D3  | embedding 預設策略      | **單一模型路徑**（預設 Qwen3-Embedding-0.6B），**不**加 model2vec fast tier                                                | 使用者選單一模型；接受首鋪是長時背景 job。fast tier 留作未來可選 lever。                      |
| D4  | **model 假設**          | **零模型假設**：dim / pooling / normalization / instruction 全部 runtime 偵測，預設模型隨時可換                            | 使用者明示 Qwen3-0.6B 只是 no-config 便利預設，未來會換（如 Qwen4-embedding）。               |
| D5  | vector 引擎             | **Turbovec**（quantized flat-scan，純 Rust，TurboQuant）藏在 `VectorIndex` trait 後；v1 同時是 flat-scan 地基與 scale 路徑 | 純 Rust 無 build 摩擦、no-codebook → model-agnostic、增量+delete+filter、維護者經使用者背書。 |
| D6  | vector fallback         | usearch、LanceDB 為 documented fallback（若 Turbovec 14.4M 延遲/成熟度不過關）                                             | 兩者皆過星門檻；benchmark 決定。                                                              |
| D7  | code execution          | **Wasmtime + Javy(JS)**，opt-in、capability-gated、只讀查詢 host API                                                       | 純 Rust 沙箱、預設零權限、僅 capable model 開啟。                                             |
| D8  | agent loop / durability | **roll-your-own thin loop**，durability 擴展既有 lease-based job queue + sidecar SQLite                                    | 2026 共識；不引入 Temporal/Restate/DBOS。                                                     |

---

## A. AI 的 storage planes（延伸 ADR-010 四層模型）

AI 不改動 canonical truth model。新增的都是**可刪除、可重建的 derived sidecar**，與 canonical SQLCipher archive 隔離：

| Plane                       | 檔案                                                          | 內容                                                               | 可重建來源                       |
| --------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------------ | -------------------------------- |
| (既有) canonical            | `archive/history-vault.sqlite` (SQLCipher)                    | 唯一 source of truth；**source text 永遠在此** → re-embed 永遠可行 | —                                |
| (既有) lexical              | `derived/history-search.sqlite` (FTS5)                        | BM25 / trigram / CJK                                               | canonical                        |
| (既有) intelligence runtime | `derived/history-intelligence.sqlite`                         | deterministic read models + **AI job queue（既有 `ai_jobs`）**     | canonical                        |
| **新** vector sidecar       | `derived/vectors/` (Turbovec index file + 可選 rerank 向量檔) | 量化向量索引（in-RAM 載入）；**向量不進 canonical SQLite**         | re-embed canonical               |
| **新** agent sidecar        | `derived/agent.sqlite`                                        | agent runs/steps journal、citation table、notes/todos、memory      | 可丟棄重來（agent trace 是衍生） |
| **新** models               | `<app-data>/models/`                                          | 下載的 embedding/reranker 模型 + 校驗 hash                         | 重新下載                         |

每個 vector / agent sidecar header 蓋 **embedding fingerprint**（見 §C.4）；mismatch → 標記 stale + 觸發重建。canonical 加密邊界、App Lock session boundary（ADR-005）對所有 AI 工具一律生效。

---

## B. LLM 接入

**B.1 形狀**：PathKeep 自有 `LlmProvider` trait 為唯一邊界，沒有任何第三方型別是 load-bearing。

```
trait LlmProvider {
  async fn chat(&self, req) -> Resp;
  async fn chat_stream(&self, req) -> impl Stream<Chunk>;   // 串 token 到 React via Tauri event
  fn capabilities(&self) -> Caps;   // tool_call? structured_output? streaming? prompt_cache? max_ctx
}
```

- model id 是 **runtime config string**（`AiProviderConfig`），永不寫進產品文案/代碼。
- **transport 後端 = 採用維護中的多-provider client，不手寫 per-provider quirks**（thinking 內容、streaming、tool calling、caching 的跨 provider 差異不該由我們長期維護）。
- **選定 `rig`（rig-core，7.7k★，MIT，0xPlaygrounds，named production users）**——它是**唯一清楚過 >6k★ 供應鏈門檻的 Rust LLM client**（無需 risk assessment），且可**只用 rig-core 的 provider/completion 層當 client**：streaming completions + 統一 tool calling（20+ provider）+ **reasoning 內容**（自 0.16 起支援、0.31 改進 reasoning-block 累積）+ structured extraction + 內建 **MCP** 支援 + OpenAI-compatible custom base_url（覆蓋 Ollama/LM Studio）。v0.39.0（2026-06-19，活躍）。**不**採用它的 agent / vector-store 框架（我們自有）。
  - **用法**：藏在自有 `LlmProvider` trait 後，adapter 內才觸碰 rig-core completion client。**不 hard-pin**——假定這種成熟大庫遵守 SemVer，且 rig 仍是 0.x（Cargo 本就把 minor bump 當 breaking → 升級是刻意動作、不會被自動帶走），用一般 Cargo 版本需求即可；把 rig 型別只關在 adapter 內（這本就是好分層）。舊整合的問題是把 rig 型別洩漏到處（那才是 throwaway 的部分），不是 rig 本身。
  - **只用 rig 的 client，不用 rig 的 `Agent` runtime**（見 §F）：rig 的 `Agent`/`.prompt()` multi-turn 迴圈是 in-memory、非 resumable、無 compaction/citation/PME-pause/取消/budget——給不了我們 agent 的硬需求。我們只取 rig-core 的 per-turn completion（chat/stream + 跨 provider tool-call 編碼 + reasoning/structured-output 正規化）。LLM functions 則只需單發 completion，不需 agent。
  - **reasoning/thinking 補強**：若某 provider 的 thinking 內容 rig 處理不足，只補那一小塊 native 萃取（bounded patch，非全 transport 手寫）。
- **被排除**：`genai`（806★）——星數不足、無個人信任 override，**不過供應鏈門檻**；`async-openai`（1.9k★）同理。
- **手寫** provider client 只作**最後手段**（rig 失修時退路），非預設。
- **provider 矩陣**：OpenAI-compatible Chat Completions 作為 **local / OpenAI-shaped 的 floor**（Ollama / LM Studio / llama-server / OpenAI / OpenRouter）；**Anthropic / Gemini 走各自 native adapter**（不走它們的 OpenAI-compat shim，否則 strict structured output 與 prompt caching 失效）。Responses API 僅作可選 richer adapter，永不作 baseline。
- **structured output**：用 provider native JSON-schema mode，但**永遠保留 serde validate-and-repair fallback**（constrained = 可解析 ≠ 語義正確）。
- **secrets**：API key 一律 `secrecy::SecretString`（drop 清零、log redact、永不序列化進 trace/sidecar）；存既有 keyring 機制。
- **degradation**：偵測 provider 不支援 tool-calling → agent 降級到 deterministic search-only 模式。**沒有任何 provider 設定時，LLM functions 退回 deterministic 輸出**（AI 純 additive）。

**B.2 tool 邊界（MCP）**：in-app 工具用 **plain Rust trait**（低開銷、緊 capability 控制）；`rmcp` **只用在對外 MCP server 邊界**（讓外部 agent 驅動 PathKeep）。任何**外部** MCP server 都是 data-egress 邊界 → 硬預設關、逐個 consent。

---

## C. Embedding

**C.1 in-app 引擎（D2）**：**candle**（HF, 20.5k★, Apache/MIT, 純 Rust CPU kernels，零下載 C++、零 cmake）。同一引擎跑 embedding 與（§E 的可選）cross-encoder reranker。MKL/Accelerate 一律 OFF（保持可重現 build）。`ort` 只作 opt-in power-user 加速 path（且必須自 vendor ONNX Runtime static build = ADR 級，非預設）。

**C.2 預設模型（D3/D4）**：no-config 預設 = Qwen3-Embedding-0.6B（Apache-2.0）。**但設計零模型假設**：

- dim、pooling（mean/last-token）、是否需 query/document instruction、是否已 normalize → **全部是 per-model capability，runtime 偵測或由 provider 描述符提供**。
- UI / 代碼**不得**寫死 1024、不得寫死 last-token、不得寫死任何 model id。
- 換模型（含未來 Qwen4-embedding 之類）是一等公民操作，見 §C.4。

**C.3 external providers**：一個自寫薄 reqwest+serde 的 **OpenAI-compatible `/v1/embeddings` adapter** 覆蓋 Ollama / LM Studio / vLLM / llama-server / 多數 cloud；只給非 OpenAI-shaped cloud（Gemini task_type/單輸入、Voyage/Cohere input_type+output_dtype）寫小 adapter。**正確性鐵律**：(a) 永遠讀實際回傳向量長度，不信 config 的 `dimensions`；(b) 一律防禦性 L2-normalize（尤其 MRL 截斷後）；(c) query/document 角色旗標穿過 trait 到每次呼叫。

**C.4 fingerprint + 失效/遷移（D4 的承載機制）**：每個 vector index 蓋 **fingerprint = hash(provider, model_id, effective_dim, output_dtype, normalized, pooling, instruction_template, version)**。

- 啟動/設定變更時比對 fingerprint；mismatch = stale。
- 換 model/dim/normalization 任一 → **整個 14.4M index 失效**。流程：PME 預覽（「換模型將重嵌 N 列，估 ~X 小時、~Y GB」）→ 背景建新 versioned index（舊 index 照常服務）→ 原子換入；遷移期可 dual-index 查詢。
- canonical 是 source text → 全量 re-embed 永遠可行；sidecar 正確地可丟棄。

**C.5 模型下載/完整性**：`hf-hub`（純 Rust）下載，但 PathKeep 自加 **SHA-256 pin + 完全離線首跑（bundle 或 sideload）+ off-thread 分片可取消 + 進度**。模型是 consent-gated，預設不下載。

**C.6 throughput 現實（最大風險，見 §03 R1）**：14.4M 首鋪在 4 核/無 GPU 上 candle+Qwen3-0.6B ≈ 數十小時~數天。**必為可暫停/續跑/限流/off-thread 的背景 job**；建索引期間 FTS5 + deterministic intelligence 正常服務。若實機 benchmark 證明太慢，model2vec fast tier（已因 D3 暫不做）是可隨時 drop-in 的未來 lever（D4 的 model-agnostic 設計使其零架構代價）。

---

## D. Vector store

**D.1 引擎（D5）**：**Turbovec**（RyanCodrai/turbovec，MIT，純 Rust + 可選 Python binding，建於 TurboQuant / Google Research ICLR 2026）。維護者經使用者明確背書 → 滿足供應鏈「reputable maintainer」門檻（記於 §I）。

實測特性（2026-06 verified, README）：

- **結構 = quantized flat-scan**（2-bit=16x / 4-bit=8x 壓縮；Lloyd-Max scalar quant 由分佈算出、**無 codebook 訓練**；TQ+ 可選 per-coord 校正在首次 add 凍結）。**無 IVF / 無 HNSW graph**。
- recall：100K/d=1536 略勝 FAISS PQ；CPU-only SIMD（NEON / AVX-512BW / AVX2 fallback）。
- 增量 insert（線上、免訓練）；**O(1) delete by external id**（IdMapIndex，uint64 穩定 id）；**allowlist/bitmask 過濾在 SIMD kernel 內**。
- persistence：`write()`/`load()` 到磁碟；**載入後全量在 RAM**（無 mmap）。
- 最大已 benchmark 規模 = **100K**；「10M→4GB」是外推宣稱、未實測。

**D.2 為何契合 PathKeep**：

- 純 Rust、無 protoc/C build 摩擦、ARM 乾淨 → 供應鏈最佳。
- **no-codebook = model-agnostic**（換 embedding 模型不需重訓量化器）+ 增量友善（持續 ingest）。
- 一個引擎同時是「~1M 以下的 flat-scan 地基」與「14.4M 的量化 scan」。
- delete + allowlist filter 直接服務 rollback/可見性過濾與 §G 的進階搜尋。

**D.3 RAM / 延遲數學（envelope 檢查）**：

- 常駐 RAM = 全量量化 index（無 mmap）。14.4M × 1024-dim：2-bit ≈ **3.7 GB**；MRL-256 2-bit ≈ **0.9 GB**；MRL-512 ≈ 1.8 GB。
- **預設用 MRL-truncated dim tier**（當模型支援 MRL；否則存 native dim）以壓常駐 RAM；full-dim 作「high-recall / 大 RAM」opt-in tier。fingerprint 記 effective_dim。
- **延遲 = O(N) scan**：每查詢掃全部量化 codes。14.4M @2-bit/1024 ≈ 掃 3.7 GB（@256 ≈ 0.9 GB），4 核 SIMD 約次秒級。對多數使用者（<1–2M）極快且近精確；14.4M 尾端是 benchmark 重點。

**D.4 14.4M 尾端 mitigations（benchmark-gated，非 v1 必需）**：

1. MRL 降維（256/512）縮小掃描量。
2. 在 Turbovec 之上疊一層**粗 IVF/centroid 預過濾**：先用少量 centroid 找最近簇，再用 Turbovec 的 **allowlist** 只掃該簇 → 變 sublinear（Turbovec 的 kernel 內過濾正是這個 hook）。
3. hot/cold split：近期(~1–3M) codes 熱常駐，長尾冷處理（60 年地平線多數查詢偏近期）。
4. 可選 disk 全精度/int8 rerank：若量化 recall 不足，對 top-K 從磁碟讀全向量重排。

**D.5 抽象與 fallback**：所有引擎藏在 `VectorIndex` trait（build/append/remove/search(query, k, allowlist?)/save/load/clear）後。

- **v1 一律先出 flat int8 scan + FTS5/hybrid 地基**（Turbovec 本身即可擔此角色）。
- **benchmark gate**（§03 R2）：真機 4 核/8GB/SSD 上量 Turbovec 在 1M/5M/14.4M 的 recall@10、查詢延遲、常駐 RAM、insert/build 時間。不過關則啟用 mitigations，仍不行則切 fallback：**usearch**（binary + in-RAM + 自管 disk-rerank；vendored-C++-in-crate；需 risk assessment）或 **LanceDB**（IVF_PQ + on-disk rerank；需解 protoc build 鏈）。

---

## E. Hybrid search + rerank

- **融合**：**RRF（k=60）為預設**（rank-only，免 normalize，解 BM25 無界 vs cosine 尺度衝突，只作用在兩個 top-K，近零成本）；**weighted/convex 為可選進階**（單一「偏關鍵字↔偏語義」旋鈕）。
- **FTS5 復用**：沿用既有 lexical recall v2；**避開 `ORDER BY rank LIMIT` 大表陷阱** → external-content FTS5 + 緊 LIMIT + 熱路徑不 join 大表。
- **rerank（opt-in，預設關）**：cross-encoder（候選家族如 Qwen3-Reranker seq-cls 單次 forward，或 bge-reranker-v2-m3）跑在 **candle**（與 embedding 共引擎），**lazy-load、用後釋放**，絕不與 embedder 同時長駐。只在候選大而雜且第一階段品質低時開（agent 自行升級）。ColBERT/late-interaction **拒絕**（multi-vector 爆儲存）。
- **agent 工具**：`search_bm25` / `search_vector` / `search_hybrid(rerank?)` 三個分開工具；`search_bm25` 在無 embedding 時也能用；agent 自 escalate（bm25 → hybrid → +rerank）。每個工具回真實 history rows（id/url/title/visited/score）。

---

## F. Agent harness

- **loop**：手寫 thin tokio while-loop（call model → 有 tool call 就執行回填 → 重複），跑在 worker，**串 token 到 UI、硬可取消、per-run step/token/cost budget、loop/卡死偵測**。provider 經 `LlmProvider`（adapter 內用 rig-core 的 per-turn completion）。**不用 rig 的 `Agent`/`.prompt()` multi-turn runtime**——它 in-memory、非 resumable、無 compaction/citation/PME-pause/取消/budget，正是本節列出的硬需求；rig 的 agent 層對我們無用，只取它的 client。
- **durability（D8）**：擴展既有 lease-based `ai_jobs` queue + **agent sidecar SQLite**（`agent_runs` / `agent_steps`，trace 是衍生、不進 canonical）。鐵律：**每個 model/tool 輸出先 journal 再 observe**；resume **replay journaled bytes、絕不重呼模型**（避免重複收費/發散）；每 tool call 有 idempotency key；PME 審批用 **long-pause** 釋放 worker。**不引入** Temporal/Restate/DBOS（皆需外部 server/RocksDB/Postgres）。
- **context engineering**：以 **recency pruning（整對丟棄 stale tool-result）+ 輕量 summarize** 為主、LLM auto-compact 為最後手段；working tokens 壓 <~32k；**帶 row id + count、evidence just-in-time、絕不 inline 大結果集**；sidecar 存 **citation table（row id → evidence）讓引用熬過 compaction**（服務透明度）。
- **sub-agents**：支援但**並發上限 ~2**、低優先、餵 pre-filtered 結果、**禁止觸發 bulk embedding**（保 4 核 UI 流暢硬指標）。
- **memory**：輕量 Anthropic memory-tool **pattern**（非重型平台），存 agent sidecar、consent-gated、可檢視/可刪、recall 重用 embedding 棧。

---

## G. Agent tools / skills / code execution（三層，預設安全、漸進解鎖）

**Layer 0 — 檢索地基（先做、永遠在、AI-optional-safe）**：~8–12 個 bounded、read-only 結構化工具，按使用者要的顆粒度：`query_visits` / `query_sessions` / `query_trails` / `query_query_groups` / `query_threads` / `query_domains` / `query_rollups` / `search_insights` / `get_insight`，各回 bounded top-K + `visit_id` provenance，外加 `fetch_visits(ids)` 驗證工具。**FTS5+bm25+trigram 是不可協商的那一個**（AI 關閉時也在）。每個結果帶 coverage/freshness metadata，index 部分/缺失時優雅降級。

**Layer 1 — 語義/hybrid（additive、consent-gated）**：§D / §E 的 `search_vector` / `search_hybrid`。

**Layer 2 — code-mode（opt-in、capability-gated 升級，D7）**：2026 Programmatic Tool Calling —— agent 寫一段 JS，在沙箱裡 fan-out 多個 BM25/vector/hybrid 查詢 + 本地 join/filter，只回蒸餾 top-K。

- **沙箱 = Wasmtime**（18.2k★，BA，純 Rust、零 C build，過門檻）+ **Javy(JS→WASM)** 預編譯 guest（agent 生成的 JS 只是餵進去的資料；不直接依賴 rquickjs、不 ship V8）。
- **能力邊界**：guest **零 ambient 權限**，唯一可呼叫的是與 Layer 0 同一組 **read-only query host fn**——**無 DB handle、無 SQL string、無 fs/net**。per-call row cap + per-script row/wall-time budget（epoch deadline，worker thread yield，UI 永不凍）。
- **能力門檻**：小模型（0.6B 預設 tier）**不開 code-mode**；capable model（雲端或本機 8B+）才開。雙路徑：弱模型用 classic single-tool-call。
- **「奇怪 properties」進階搜尋**：受限、參數化、read-only（`PRAGMA query_only`/immutable）、跑在 derived/intelligence sidecar 的 Text-to-SQL host fn；**絕不碰 canonical SQLCipher**。

> **更新（2026-06，使用者決策 — 推翻上方「opt-in、capability-gated」）**：code-mode 改為 **預設開啟（default-enabled）**，**取消模型能力門檻**。理由：**Wasmtime 沙箱本身就是安全邊界**（零 ambient 權限、read-only host fn、epoch wall-time deadline + memory/host-call/output 硬上限），自動執行在限額內是安全的；且 LLM 是可替換的（雲端或本機皆可），因此不再用「0.6B 不開 / 8B+ 才開」之類的能力門檻 gate code-mode。W-AI-8 WU-2 落地時，`run_code` 工具與四個搜尋工具一起無條件註冊進每個 agent run（`with_default_search_tools()`）。本段上方 Layer 2 標題的「opt-in、capability-gated」描述以本註記為準。

**Skills（SKILL.md / agentskills.io）**：用在**對外 MCP 面**（外部 agent 透過 progressive disclosure 學「顆粒度階梯 + BM25-vs-vector-vs-hybrid + 引用 visit id」）；**PathKeep 自家小模型烘一個緊湊 compiled prompt 即可**，不跑多檔 skill 協議。skills 與 MCP tools 互補不互斥。

**透明度**：code-mode 對模型隱藏中間結果，但**使用者必須看到** agent 跑了什麼——UI 顯示執行的 script、每個 host-query、引用的 visit id（對齊 PME / evidence 契約）。

---

## H. 跨領域：consent / gating / 透明 / i18n

- 所有 AI **預設關、consent-gated、provider 可配**；**無 LLM 無 embedding 時 app 完整可用**（deterministic intelligence + FTS5）。
- 任何外部 provider / 外部 MCP server = egress 邊界 → 硬預設關、逐個 consent、PME 預覽顯示目的地 host。
- 任何 AI 回答 **必引用真實 history rows**，深鏈回 Explorer canonical filters；透明優先於黑盒。
- 所有 user-visible copy（含 aria-label、空/錯/載入/禁用/consent 狀態）開發當下交付 `en`/`zh-CN`/`zh-TW`；**文案不寫任何 model id**。
- 前端流暢硬指標：embedding/rerank/code-exec 一律 off-thread、分片、可取消、有 skeleton/進度；主線程永不凍。

---

## I. 供應鏈 ledger（採用前的信任裁定）

| 依賴                                | 角色                         | 裁定 / 所需動作                                                                                                                                                                                                 |
| ----------------------------------- | ---------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| candle (20.5k★)                     | in-app embedding+rerank 引擎 | ✅ 過門檻，直接採用                                                                                                                                                                                             |
| Wasmtime (18.2k★)                   | code-mode 沙箱               | ✅ 過門檻                                                                                                                                                                                                       |
| rmcp (official)                     | 對外 MCP 邊界                | ✅ reputable-org 例外                                                                                                                                                                                           |
| Javy (BA, 2.7k★)                    | JS→WASM guest                | ✅ BA org                                                                                                                                                                                                       |
| **Turbovec (~3.5k★, 單人)**         | vector 引擎                  | ✅ **維護者經使用者(專案擁有者)明確背書 → 滿足「reputable maintainer」門檻**；仍須：pin 版本、vendor Cargo.lock、§03 R2 成熟度+benchmark gate                                                                   |
| **rig / rig-core (7.7k★)**          | LLM transport（選定）        | ✅ 過 >6k★ 門檻，無需 risk assessment；**只用 completion client 層**（不用其 `Agent`/vector-store 框架）、藏 `LlmProvider` trait 後；假定遵守 SemVer，不 hard-pin（0.x→Cargo 已把 minor 當 breaking，升級刻意） |
| genai (806★) / async-openai (1.9k★) | LLM transport                | ❌ 星數不足、無個人信任 override → 不過供應鏈門檻                                                                                                                                                               |
| usearch (4.2k★)                     | vector fallback              | ⚠️ 需 risk assessment（採用時）                                                                                                                                                                                 |
| LanceDB/Lance (6.7k★)               | vector fallback              | ⚠️ 過星門檻，但採用需先解 protoc/C build 鏈                                                                                                                                                                     |
| ort                                 | embedding power-path         | ❌ 預設拒絕（default download-binaries 禁用；static-link 需自 vendor ONNX = ADR 級）                                                                                                                            |
| LEANN / sqlite-vec / faiss-next     | —                            | ❌ 見 01 §4                                                                                                                                                                                                     |

通則：`secrecy::SecretString` 存所有 API key；新 native build 必須 vendored-in-crate（rusqlite 模式）或純 Rust。

---

## J. 待寫 ADR / benchmark gate（採用前）

1. **ADR：vector 引擎選型** —— 在真機 4 核/8GB/SSD 跑 Turbovec(主) vs usearch/LanceDB(fallback) 的 recall@10 / 延遲 / RAM / build 時間 @1M·5M·14.4M；含 Turbovec 成熟度審計（14.4M 規模、persistence 健壯性、delete/filter 正確性、API 穩定度）。→ §03 R2。
2. **rig 升級策略**：rig 已過供應鏈門檻（7.7k★），無需 risk-assessment ADR；假定遵守 SemVer → 不 hard-pin、用一般 Cargo 版本需求（0.x→minor 升級刻意）；rig 型別只在 adapter 內、外層維持自有 `LlmProvider` trait；**只採 rig client 層，agent 迴圈自有**；手寫 client 僅作 rig 失修退路。
3. **ADR（若 fallback 觸發）：LanceDB protoc/C build 鏈 vendoring** 或 **usearch C++ FFI risk assessment**。
4. **benchmark：embedding 首鋪吞吐**（candle+預設模型，4 核）→ 決定是否需重啟 model2vec fast tier（D3 回檔點）。→ §03 R1。
5. **ADR：code-mode capability-detection**（哪些模型開 code-mode；弱模型 fallback 判定）。
6. **ADR：agent 沙箱 host-API 表面凍結**（read-only query 契約，對 in-app 與 MCP 兩面一致）。

實作分階段見 [03-implementation-plan.md](03-implementation-plan.md)。
