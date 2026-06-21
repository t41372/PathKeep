# AI Redesign 2026 — Implementation Plan

> 承 [02-architecture-decisions.md](02-architecture-decisions.md)。分階段、可驗收、風險前置。
> 鐵律：每階段改了代碼就跑 `bun run check`（權威 gate），過了才提交；功能改動同步回寫 `docs/features/` 與 `docs/architecture/`。
> 這是 proposal 階段的計畫文檔；正式啟動時把各 milestone 拆成 `STATUS.md` 的 work block。

---

## 0. 排序哲學

1. **先地基、後模型**：先把 trait 邊界、storage planes、consent/gating、config schema、secrets 立穩（不碰任何 ML），再逐層加 AI。
2. **AI 永遠 additive**：每個 milestone 結束時，關掉 AI 後 app 必須完整可用（deterministic intelligence + FTS5）。
3. **風險前置**：兩個 benchmark gate（R1 embedding 吞吐、R2 vector 規模）做成**早期 spike**，先用真機數字驗證再投入完整管線。
4. **既有 AI 代碼**：rig.rs / LanceDB / ai_sidecar / 舊 provider schema 視為可替換 legacy；本計畫逐步以新 trait 取代，不擴展舊 contract。受保護入口（`src/main.tsx`、`src/lib/ipc/bridge.ts`）改動需對齊既有 contract。

---

## 1. 早期 spike（風險前置，先做）

| Spike                 | 對應風險 | 做什麼                                                                                                                                                                            | 通過標準                                                                                              |
| --------------------- | -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| **S1 embedding 吞吐** | R1       | candle + 預設模型，在真機 4 核/8GB 上量 batched docs/sec 與 peak RAM；估 14.4M 首鋪 wall-clock                                                                                    | 有可重跑 artifact + 14.4M ETA 數字；決定是否需重啟 model2vec fast tier                                |
| **S2 Turbovec 規模**  | R2       | Turbovec 在 1M/5M/14.4M(合成) 上量 recall@10 / 查詢延遲 / 常駐 RAM / insert·build 時間；含成熟度審計（persistence 健壯性、delete/filter 正確性、API 穩定度、無 mmap 的 RAM 行為） | 有 artifact；確認 14.4M 是否在 envelope 內，或需 mitigation(MRL/IVF-prefilter/hot-cold) 或切 fallback |

兩個 spike 的 artifact 存 `artifacts/benchmarks/`（對齊 `PG-RD-OUT-002`：保留輸入、命令、環境、結論）。

---

## 2. Milestones

### M-AI0 — Foundations & contracts（無模型）

**目標**：立穩所有邊界，舊 AI 代碼可被新 trait 取代。
**讀先**：02 全文、`module-boundary-map.md`、`desktop-command-surface.md`、ADR-005/010/006。
**work**：

- 定義 `LlmProvider`、`EmbeddingProvider`、`VectorIndex` traits（vault-core）。
- 重寫 `AiProviderConfig` / `AiSettings` 為 **model-agnostic**（per-model capability 描述符：dim/pooling/normalized/instruction/role；無寫死 id）。
- 新 storage planes 落地（`derived/vectors/`、`derived/agent.sqlite`、`<app-data>/models/`）；fingerprint header 結構。
- API key 改 `secrecy::SecretString`；沿用 keyring。
- consent/gating 骨架：所有 AI 預設關、provider 未配時 deterministic fallback；UI roadmap/disabled 狀態（i18n 三語）。
  **驗收**：`bun run check` 綠；關 AI 時全 app 行為不變；舊 ai_sidecar stub 行為由新 trait 的 no-op 實作接管。

### M-AI1 — External LLM transport + LLM functions

**目標**：純 external chat / LLM functions（無 in-app 生成）。
**讀先**：02 §B。
**work**：

- `LlmProvider` 後端（genai 風評估+pin+vendor，或手寫）；OpenAI-compat floor + Anthropic/Gemini native adapter + capability/connection probe。
- structured output（native schema + serde repair fallback）；streaming 串 token 到 React。
- 第一批 **LLM functions**（可降級）：topic/domain/query-family 摘要敘述、query expansion、entity hint —— 無 provider 時退 deterministic。
  **驗收**：connection test 回 capability/latency/error；無 provider 時 functions 退 deterministic；`bun run check` 綠。
  **gate**：genai risk-assessment ADR（§02 J.2）或手寫決定。

### M-AI2 — In-app embedding + external embedding + backfill

**目標**：算得出向量、存得進 sidecar、換模型不崩。**（含 R1 spike 結果）**
**讀先**：02 §C、S1 artifact。
**work**：

- candle in-app 引擎（embedding）；模型下載（SHA-256 pin、離線、off-thread、可取消、進度）。
- OpenAI-compat `/v1/embeddings` adapter + Gemini/Voyage/Cohere 小 adapter；正確性鐵律（讀實際 dim、防禦 normalize、query/doc role）。
- fingerprint + 失效偵測 + PME 重嵌遷移（versioned index、dual-index 查詢、原子換入）。
- **backfill job**：擴展既有 lease-based queue，chunked / resumable / 限流 / off-thread / 進度；建索引期 FTS5 照常服務。
  **驗收**：14.4M(或合成) 首鋪可暫停續跑、UI 不凍；換模型走完整 PME 重嵌；`bun run check` 綠 + coverage gate。
  **gate**：R1 —— 若吞吐不可接受，啟動 model2vec fast tier 回檔點（D3）。

### M-AI3 — Vector store + semantic search

**目標**：語義檢索上線。**（含 R2 spike 結果）**
**讀先**：02 §D、S2 artifact。
**work**：

- `VectorIndex` trait + **Turbovec** 實作（build/append/remove/search(allowlist)/save/load/clear）；MRL dim tier；in-RAM 載入 + sidecar 持久化。
- `search_vector`；結果回真實 rows + score。
- 視 R2 結果啟用 mitigations（MRL 預設 dim / IVF-prefilter via allowlist / hot-cold）或切 fallback（usearch/LanceDB）。
  **驗收**：1M 級即時、14.4M 在 envelope 內（或 mitigation 後達標）；常駐 RAM 在預算內；`bun run check` 綠。
  **gate**：R2 vector 選型 ADR（§02 J.1）。

### M-AI4 — Hybrid search + (optional) rerank

**讀先**：02 §E、`lexical-recall-v2.md`。
**work**：

- RRF(k=60) 融合 + 可選 weighted 旋鈕；FTS5 復用且避 `ORDER BY rank` 大表陷阱（external-content + tight LIMIT）。
- `search_bm25` / `search_hybrid(rerank?)`；rerank = candle cross-encoder，lazy-load/用後釋放，opt-in。
  **驗收**：三工具各自可用；`search_bm25` 在無 embedding 時也工作；rerank 開關不影響基礎路徑；`bun run check` 綠。

### M-AI5 — Agent harness

**讀先**：02 §F。
**work**：

- thin tokio loop（streaming、硬取消、step/token/cost budget、loop 偵測）。
- durability：擴展 `ai_jobs` + `agent.sqlite`（runs/steps、journal-before-observe、replay bytes、idempotency、long-pause PME）。
- context engineering（recency pruning + summarize、citation table、<32k working）；sub-agents 並發≤2；memory-tool pattern。
- 工具 = plain Rust trait（Layer 0/1）；agent 回答強制引用真實 rows。
  **驗收**：長 run 可崩潰續跑且不重呼模型/不重複收費；UI 不凍；引用熬過 compaction；`bun run check` 綠。

### M-AI6 — Code-mode（Layer 2）

**讀先**：02 §G。
**work**：

- Wasmtime + Javy(JS) 沙箱；host 只暴露 read-only query API（無 DB handle/SQL string/fs/net）；row cap + epoch budget。
- 受限 read-only Text-to-SQL host fn（derived sidecar、`PRAGMA query_only`）。
- capability gating：capable model 才開 code-mode；弱模型走 classic tool-call。
- 透明 UI：顯示執行 script / 每個 host-query / 引用 visit id。
  **驗收**：沙箱無法觸網/碰 fs/碰 canonical DB；runaway script 被 budget 終止、UI 不凍；prompt-injection 基本紅隊不外洩；`bun run check` 綠。

### M-AI7 — MCP face + skills + consent UX + i18n

**讀先**：02 §B.2/§G/§H。
**work**：

- rmcp 對外邊界硬化（read-only/bounded/budgeted 契約、與 in-app host API 一致）；外部 MCP server 逐個 consent。
- SKILL.md skills（對外 MCP 面：顆粒度階梯 / BM25-vs-vector-vs-hybrid / 引用 visit id）；in-app 小模型用 compiled prompt。
- consent / provider 配置 / 重嵌預覽 等全 UI 的三語 copy 與非顯眼狀態。
  **驗收**：外部 agent 能透過 MCP+skill 正確驅動且受同一 bounded/read-only 契約；i18n 三語齊；`bun run check` 綠。

### M-AI8 — Hardening & evidence

**work**：14.4M 全開 profiling（React Profiler / flamegraph）；prompt-injection 紅隊；packaging/release-size audit；supply-chain evidence（genai/Turbovec/usearch/LanceDB 視採用）；更新 `docs/features/intelligence*.md` 與 `docs/architecture/`、ADR closeouts。
**驗收**：真機 14.4M「全 AI 開」流暢度有 artifact 背書（不憑猜）；`bun run verify` 通過。

---

## 3. 依賴順序

```
M-AI0 ─┬─ M-AI1 (LLM functions, 可獨立交付)
       └─ M-AI2 ─ M-AI3 ─ M-AI4 ─┐
                                  ├─ M-AI5 ─ M-AI6 ─ M-AI7 ─ M-AI8
S1 ↘ gate M-AI2     S2 ↘ gate M-AI3
```

M-AI1（external LLM functions）可在 embedding/vector 之前先交付價值。agent（M-AI5+）依賴檢索層（M-AI3/4）。

---

## 4. 風險登記

| #      | 風險                                                                               | 影響                        | 緩解                                                                                    |
| ------ | ---------------------------------------------------------------------------------- | --------------------------- | --------------------------------------------------------------------------------------- |
| **R1** | 14.4M 首鋪 embedding 在 4 核/無 GPU = 數十小時~數天                                | 使用者長時間沒有語義搜尋    | 永遠背景/可續跑、FTS5 同時服務；S1 量測；model2vec fast tier 回檔點                     |
| **R2** | Turbovec O(N) scan + in-RAM 在 14.4M 尾端延遲/RAM 超預算（且僅 benchmark 到 100K） | 大檔使用者語義搜尋慢/超 RAM | S2 spike + 成熟度審計；MRL/IVF-prefilter/hot-cold mitigations；usearch/LanceDB fallback |
| R3     | code-mode prompt-injection（私有歷史 + 不可信 page-title/URL token）               | 資料外洩                    | 沙箱零 ambient 權限、只讀查詢 API、無 net/fs、budget；紅隊；capability-gated            |
| R4     | genai 806★/單人 bus-factor                                                         | transport 斷更/breaking     | risk assessment + pin + vendor + 自有 trait 包一層；可換手寫                            |
| R5     | 換 embedding 模型使 14.4M index 全失效                                             | 長重嵌、UX 衝擊             | fingerprint 偵測 + PME 預覽 + 背景 dual-index 遷移 + 原子換入                           |
| R6     | 前端被 embedding/rerank/code-exec 凍住（曾有 ~10s 凍結前科）                       | 流暢度硬指標破功            | 全 off-thread/分片/可取消/skeleton；M-AI8 profiling 背書                                |
| R7     | 與 `feat/intelligence-overhaul` 衝突（另一 agent 並行改 intelligence）             | 合併摩擦                    | 本工作隔離於 `feat/ai-redesign-2026`；落地前與 intelligence overhaul reconcile          |

---

## 5. 與既有 gate 對齊

- 每 milestone：`bun run check` 為 per-commit 權威 gate；100% coverage gate（`coverage:js`/`coverage:rust`）。
- 性能敏感改動：profiling artifact 後才宣稱「夠快」（不憑猜）。
- 文檔：改功能更新 `docs/features/`，新技術決策更新 `docs/architecture/` + ADR；benchmark 保留可重跑 artifact。
- macOS Rust gate 的 `/dev/shm` 限制是已知非回歸項，不阻擋本計畫。

---

## 6. 不在本計畫範圍

雲端託管 AI / 帳號 / 跨機同步；用 AI 取代 deterministic intelligence 或 lexical recall；改 canonical 加密邊界；in-app 生成 LLM（D1 已排除）；文案寫死 model id。
