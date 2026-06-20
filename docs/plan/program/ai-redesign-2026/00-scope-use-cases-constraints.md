# AI Redesign 2026 — Scope, Use Cases & Constraints

> Frame 文檔。在研究與架構決策之前，先把「我們要做什麼、為誰做、邊界在哪、什麼不可動」講清楚。
> 既有 AI 實作（rig.rs / LanceDB / ai_sidecar / 既有 provider schema）**僅供參考、不約束本設計**。

---

## 1. 我們要接入的兩種 AI

使用者明確區分了兩類 AI surface，兩者共用底層的 LLM / embedding / retrieval 基礎設施，但產品形態與工程要求不同：

### A. LLM functions（輔助既有功能的「點狀」AI）
散佈在各功能裡的、單次、結構化、可預期的 LLM 調用。例如：
- topic / cluster 命名、domain 或 query family 的人類可讀摘要
- insight 卡片的自然語言敘述（deterministic 已算出證據，LLM 只負責可讀性）
- query expansion / 意圖改寫（輔助 search，不取代 lexical）
- 結構化抽取（從 title / URL 推斷 entity、語言、分類提示）

要求：**可降級**（沒有 LLM 時功能仍在，只是少了敘述層）、結構化輸出（JSON schema / 受約束解碼）、快、可快取、可審計。

### B. 專屬 chatbot agent（長時運行的對話式 agent）
一個能滿足使用者各種「奇怪請求」的 agent，採 2026-06 的 agent 最佳實踐。能力包含（使用者點名的）：
- 以**不同顆粒度**查瀏覽歷史：raw visits、sessions、search trails、query groups、threads、domains、daily rollups、refind pages、insights……
- 一個 turn 內**並行調用多個檢索工具**：embedding search / hybrid search / BM25 / advanced property search
- 取用更**細顆粒**的條目資料、檢索 / 查看 insights
- 用**奇怪的 properties** 做進階 / 組合搜尋
- **寫 Python / JS 代碼**輔助搜尋與分析（code execution as a tool）
- **長時間運行**，滿足開放式請求

開放問題：是否把內建工具打包成 **Agent Skills**（SKILL.md 模式）？code execution 沙箱深度多深？——留待研究後決策（見 §6）。

---

## 2. 要設計的子系統

| 子系統 | 範圍 | 預設傾向（待研究確認） |
| --- | --- | --- |
| **LLM 接入** | 2026 做法（非 litellm-2023）。統一 provider 抽象，支援 external（Ollama / LM Studio / OpenAI-compatible / cloud）與可選 in-app 推理；first-class tool calling / streaming / structured output。 | 不寫死 model id；external-first，in-app LLM 為可選 |
| **Embedding：in-app 推理** | 在 app 內、Rust、CPU 上跑 embedding，作為**預設路徑**。 | 預設模型：Qwen3-Embedding 0.6B 家族 |
| **Embedding：external providers** | LM Studio / Ollama / OpenAI-compatible / cloud，model-agnostic、SOTA-agnostic 抽象。 | 與 in-app 走同一抽象 |
| **Vector store** | ~14.4M 向量、8GB RAM、無 GPU、可重建 on-disk sidecar（不進 canonical SQLite）。多看方案：LEANN、Turbovec、sqlite-vec、Lance、hnsw_rs、usearch、DiskANN、量化策略…… | 影響很大，需深入比較 |
| **AI agent** | agent loop、durable / long-running、context engineering、tools、skills、code execution。 | 復用既有 lease-based job queue 做 durability |
| **Hybrid search + rerank** | FTS5/BM25 + dense + 可選 reranker，融合（RRF/weighted），以 agent tools 暴露。 | 復用 FTS5 與 in-app 推理引擎 |

---

## 3. 硬約束（不可動，與 AI 決策無關的產品事實）

1. **性能信封**：目標機 4 核 3GHz / 8GB RAM、**無 GPU 保證**。需扛 ~1440 萬 visit rows（60 年）與同量級一次性導入。對 1440 萬條做 embedding / 建索引 / 服務檢索，都得在此信封內。**前端任何時刻不准凍結主線程**——重活切出主線程、分片、增量。
2. **Storage truth model**：canonical archive 是**唯一 source of truth**（SQLCipher 加密 SQLite）。所有 AI 產物（embeddings、向量索引、摘要、agent traces）都是**可刪除、可重建的衍生狀態**，放獨立 sidecar。**向量 payload 不進 canonical SQLite**。可用 SQLite：3.50.x + FTS5 + trigram + bm25。團隊對「在 canonical DB 用 loadable extension」保持警惕。
3. **Native-dependency / supply-chain 規則**：優先選 vendored/bundled C/C++ 後端的 Rust crate（cargo build 時編譯，標準做法、免額外審批）；次選 vcpkg manifest mode。禁止：靠 Homebrew/apt/winget/global pkg-config 找原生庫；在已發布 crate 之外 download/clone/編譯 C/C++ 的 build.rs；用低信任 binding 抄捷徑；只在開發機能用的動態庫路徑。新依賴需信任訊號（GitHub stars > ~6k 或知名維護者/組織），否則先寫風險評估。
4. **Local-first / data sovereignty**：預設不出網。AI **可選、預設關閉、consent-gated、provider 可配**。**沒有 LLM、沒有 embedding 時 app 仍完整可用**（deterministic intelligence 已覆蓋核心）。AI 純屬 additive。
5. **文案不寫具體 model id**：SOTA 模型更新太快，設計必須 model-agnostic。研究中可點名當下 SOTA 模型以理解 landscape，但 UI / 設計不得寫死。預設 in-app embedding 模型為 Qwen3-Embedding 0.6B 家族（使用者沒指定時）。
6. **Evidence / 透明**：任何 AI 回答都要能引用真實 history rows（historyId / profileId / URL / title / visited time / score 等），深鏈回 Explorer canonical filters；透明優先於黑盒。
7. **i18n**：所有 user-visible copy（含 aria-label、空/錯/載入/禁用狀態）開發當下即交付 `en` / `zh-CN` / `zh-TW`。

---

## 4. 可重用的既有基礎設施（reference，可沿用或替換）

> 這些是「機制」層，多半可沿用；具體的 AI 框架 / vector 選型才是要重做的。

- **Durable AI job queue**：lease-based CAS queue（heartbeat / lease_owner / lease_expires_at / stop_requested），cooperative cancel、retry/backoff、pause/resume、manual replay。→ 天然支撐「long-running agent」與「背景 embedding 重建」的 durability。
- **MCP server**：已有可運作的 stdio MCP server（rmcp），暴露 `search_history` 工具、走統一 run ledger 審計、尊重 App Lock / 可見性過濾。→ agent 工具暴露的既有基礎。
- **Provider config / secret 機制**：provider 配置 + keyring secret 儲存（macOS Keychain / Windows / Linux Secret Service）。→ 抽象要重做，但 secret 機制可沿用。
- **Lexical recall v2（FTS5）**：ICU4X NFKC + OpenCC 繁簡 folding + CJK grams + trigram + bounded edit-distance typo fallback + BM25 ranking + 進階運算子（site:/intitle:/after: 等）。→ hybrid search 的 BM25 半邊已就緒。
- **Deterministic intelligence**：sessions / query groups / threads / refind / domain rollups / insights 等 read models，無 LLM 即可產出。→ agent 工具要查的「不同顆粒度」資料多半已存在。
- **Run ledger + evidence/citation contract**：統一審計與引用欄位契約。

---

## 5. 非目標（Non-goals）

- 不做雲端託管 / 帳號 / 跨機同步的 AI。
- 不用 AI 取代 deterministic intelligence 或 lexical recall；AI 是 additive 層。
- 不為了 AI 把 canonical archive 的 truth model 或加密邊界改掉。
- 不在文案 / 設計裡寫死任何 model id。
- 本提案階段**只產出研究 + 架構 + 實作計畫文檔，不改產品代碼**。

---

## 6. 待定 decision forks（研究後與使用者確認）

研究結論到位後，這些是會「改變後續做法」的高槓桿選擇，需明確 sign-off：

1. **in-app LLM 推理的範圍**：embedding 一定要 in-app（預設）；但 chatbot / LLM functions 的**生成模型**是否也要支援 in-app 本地推理（在 8GB 上跑小 LLM），還是 LLM 一律走 external/user-configured provider，in-app 僅 embedding？（影響是否打包 GGUF runtime、模型下載 UX、RAM 預算。）
2. **vector store 範式**：recompute-based（LEANN 式，省儲存、吃 in-app 推理吞吐）vs. 量化 on-disk ANN（usearch/DiskANN 式）vs. 其他——取決於研究的 8GB×14.4M 實測數字。
3. **code execution 沙箱深度**：agent 能寫 Python/JS 到什麼程度、跑在什麼沙箱（Deno / QuickJS / WASM / Pyodide / 子進程隔離）、能碰哪些資料（只讀本地查詢 API？）。
4. **tools vs. skills 打包**：是否把內建工具/工作流打包成 Agent Skills（SKILL.md），以及 skills 與 MCP tools / code execution 的關係。
5. **reranker**：是否引入 in-app CPU reranker（cross-encoder）做 hybrid 第二階段重排，是否值得在我們的 scale 上付這個延遲。

---

## 7. Use case 速記（agent 要能滿足的代表性請求）

- 「我去年研究 X 的時候都看了哪些網站？把過程還原一下，按時間線。」
- 「找出我反覆回去看但很久沒開的參考頁。」
- 「比較我這個月和上個月在某類網站上的瀏覽模式。」
- 「用語義搜尋找跟『分散式系統共識』相關的瀏覽，再用 BM25 交叉驗證，給我合併排序的結果。」
- 「寫段代碼統計我每個 domain 的訪問時段分佈，畫個熱力表。」
- 「我大概在某個週末看過一篇講 Rust async 的長文，標題忘了，幫我找回來。」（模糊、跨工具、需要長時間檢索）

這些共同點：**多顆粒度、多工具、可能要寫代碼、可能長時間運行、結果必須能溯源到真實 history rows。**
