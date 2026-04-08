# INTELLIGENCE — 洞察

> 從 [vision-and-requirements.md](../vision-and-requirements.md) 抽出。  
> Intelligence 是建立在 Archive 夠好的基礎上的增值層。  
> 所有 AI 功能預設關閉，可以在設定中開啟。

---

## 1. 語義搜尋（Semantic Search）

**作為**用戶，**我想要**用自然語言搜尋我的歷史紀錄，**以便**能找到「我記得看過某個講 local-first 的文章但記不得具體名字」這類模糊記憶。

### 需求要點

- 基於 embedding 的向量相似度搜尋，使用 rig.rs 驅動 embedding pipeline，LanceDB 作為向量存儲和 ANN 索引。
- Embedding 增量計算、本地索引、避免重算。
- 搜尋不只找頁面，還要支持找 session、task 和 topic level 的語義匹配。
- day-one recall mode 明確區分 `keyword`、`semantic`、`hybrid`；semantic / hybrid 必須顯示目前使用的 provider / model / index state，語義檢索不可用時要明講退化成 keyword recall。
- v1 semantic result 以 canonical visit evidence 為核心：至少回傳 `historyId`、profile / browser、URL / title、visited time、match reason、score band，並能 deep-link 回 Explorer 查原始記錄。

---

## 2. AI 助手（Ask My History）

**作為**用戶，**我想要**用自然語言問我的歷史紀錄問題，**以便**能在不手動翻閱的情況下回顧過去的研究。

### 需求要點

- 基於 LLM 的問答，context 來自 archive 的 agentic RAG 檢索（rig.rs 驅動）。
- 問的不是網際網路，而是「我的過去」：
  - 「我什麼時候開始研究 MCP 的？」
  - 「我上次比較 vectorDB 方案時看了哪些東西？」
  - 「我最近三個月在哪些領域花了最多時間？」
- Agentic search：LLM 可以多步檢索、比較、歸納。
- 回答必須附帶 evidence（哪些歷史紀錄支持了這個結論）。
- 這是**顯式觸發**的功能，不是背景常駐。
- assistant response state 必須誠實可見：`queued`、`completed`、`insufficient-evidence`、`failed`、`cancelled`，並保留 `jobId` / `runId` / provider snapshot 供 audit trace 與 queue reload。
- citation contract 至少包含 `historyId`、`profileId`、URL / title、visited time、score；若 citation ranking 後沒有可保留 evidence，assistant 必須拒答，而不是補一個看似合理的答案。
- queued assistant request 可在執行前 replay / cancel；目前 running job 仍不支援 mid-flight cancel，UI 必須清楚說明這個邊界。

---

## 3. 外部 AI 工具整合

**作為**用戶，**我想要**能讓外部 AI 工具存取我的瀏覽歷史，**以便**在其他 AI 助手中也能搜尋和利用我的歷史紀錄。

### MCP Server

- 在設定中手動開啟。
- App 啟動本地 MCP server。
- 提供搜尋、檢索歷史紀錄的 MCP tools。
- 安全考量：只綁定 localhost，不對外暴露。

### AI IDE Skill

- 提供一個 skill 定義檔（markdown 格式的指令文件），讓 AI coding assistant（如 Cursor, Gemini CLI, Copilot 等）能夠理解如何使用 MCP server 搜尋用戶的歷史紀錄。
- Skill 描述：可用的 MCP tools、典型查詢範例、回傳資料格式。
- 搭配 MCP server 使用 — skill 告訴 AI「怎麼問」，MCP server 負責「怎麼答」。

---

## 4. 洞察系統（Insights）

**作為**用戶，**我想要**看到基於我的長期歷史紀錄生成的洞察，**以便**理解自己的興趣演化、工作模式、和注意力走向。

### 架構原則

洞察系統採用**模塊化設計**：每個洞察功能是一個獨立的模塊，有自己的數據需求、計算邏輯和 UI 呈現。

- 新的洞察可以獨立開發和添加，不影響現有功能。
- 用戶可以選擇開啟/關閉個別洞察模塊。
- 不同模塊可以有不同的計算頻率和觸發條件。
- 第三方未來可以貢獻新的洞察插件。

每個洞察模塊定義：

- **名稱和描述**
- **數據依賴**：需要哪些表、哪些 enrichment、是否需要 embedding
- **計算邏輯**：怎麼從原始數據算出洞察結果
- **觸發條件**：什麼時候重算（每次備份後、每天、每週、手動）
- **UI 組件**：怎麼展示

### 分析管線

```
層 1：結構特徵提取
  從每條歷史紀錄中提取可計算的特徵：
  URL 結構、domain 類型、搜尋關鍵詞、transition/referrer、
  訪問時間/星期/時段、訪問頻率、估計停留時長

層 2：Session 和 Task 構建
  把零散的頁面訪問聚合成有意義的單元：
  相鄰訪問 → session → 語義相近的 session 合併為 thread → 偵測 reopen

層 3：Topic 聚類和時間序列
  對 visit/session 的 embedding 做聚類，形成 topic →
  追蹤每個 topic 隨時間的變化 → 偵測趨勢、爆發、轉折

層 4：LLM 增強
  為 topic 和 thread 起人類可讀的名字 →
  生成對比式摘要 → 解釋為什麼某個洞察值得關注
```

### V1 洞察功能

##### 🕐 歷史上的今天（On This Day）

讓用戶看到過去幾年的同一天，自己在瀏覽什麼。

- 拉出歷年同一天（±1 天容差）的歷史紀錄，按年份分組展示。
- 同一天的比對以使用者目前系統 timezone 的本地日曆日為準，不用 raw UTC 日期切片假裝對齊「今天」。
- 如果有足夠數據，用 LLM 生成一句話摘要。
- 適合放在 Dashboard 上作為每日亮點。
- **實現**：純數據庫查詢，不需要 embedding。有 LLM 時可生成摘要，沒有也能用。

##### 📊 定期總結（Periodic Summaries）

自動生成日度、週度、月度、年度的瀏覽總結。

- **日度總結**：今天你主要在研究什麼？訪問了多少個頁面？最活躍的 domain 是什麼？
- **週度總結**：和上週相比，你的研究重心有什麼變化？本週新出現了什麼主題？
- **月度總結**：本月的主題分布、最深入的研究線、最常用的資訊來源。
- **年度總結**：年度回顧 — 你這一年的注意力分布、主要的研究階段和轉折點。
- **實現**：統計部分不需要 AI。主題歸納和對比式描述使用 LLM。

##### 🌊 Topic Timeline（主題時間軸）

你最近在關注什麼？哪些主題在升溫？哪些在冷卻？

- 以視覺化的方式展示主題隨時間的變化。
- 用戶可以點擊任何一個主題，看到具體頁面和時間分布。
- **實現**：對歷史紀錄做 embedding，用增量聚類算法分成 topic cluster。用 LLM 給每個 cluster 起名。

##### 🧵 Task / Thread Detection（任務和研究線偵測）

自動把歷史紀錄切成一個個有意義的任務。

- 自動偵測進行中的研究線：跨天持續的、語義上連貫的一系列訪問。
- 偵測「任務重新打開」：停了幾天又回來繼續的研究線。
- **實現**：先切出 session（≤30 分鐘間隔），再用 embedding 相似度合併語義連貫的 session 成 thread。

##### 🔄 Open Loops（未完成的任務）

找出反覆在看但一直沒完成的事情。

- **實現**：基於 thread 的 revisit 次數和是否出現收斂信號。

##### 💎 Important but Unsaved（重要但沒保存的頁面）

你看了很多次、顯然覺得重要，但從來沒有 bookmark 的頁面。

- **實現**：`importance = revisit_count × estimated_dwell × semantic_centrality`。

##### 📈 Explore vs Exploit（探索 vs 深挖）

你最近是在到處看新東西，還是在集中鑽研某個問題？

- **實現**：計算 domain Shannon entropy、新 domain 佔比、revisit concentration。

##### 🗺️ Source Role Map（資訊來源角色圖）

根據實際使用方式，把常用網站分成角色：搜尋入口、社群探索、問題定位、學習消費等。

##### 🔍 Query Reformulation Ladder（搜尋問題的演化路徑）

分析同一研究線中搜尋關鍵詞的演化方向。目前只有 Chromium 系瀏覽器能提供這個數據。

##### 🌐 Site Analytics（網站統計）

按 domain 統計訪問次數和估計 session 時長。純數據庫查詢和統計計算，不需要 AI。

##### 🎯 Contrastive Summary（對比式摘要）

「這週 vs 上週，你的研究重心變了什麼？」把結構化差異交給 LLM 寫成人話。

### V1 洞察顯示契約

- 每張 insight card 都必須顯示生成時間、資料視窗、evidence 數量，以及是否依賴 Chromium-only enhancement。
- On This Day、Site Analytics、Periodic Summary、Topic Timeline 都必須能 deep-link 回 Explorer evidence，或帶著 scoped question 跳進 Assistant。
- explainability panel 必須可列出該 insight 使用的 evidence 與補充 notes，不能只顯示一段摘要。
- zero-data、新 archive、AI disabled、index rebuilding、provider unavailable 等情境都必須回傳 honest fallback，而不是合成看似完整的 insight。

### V1.5+ 洞察功能

以下功能放在 V1 之後迭代，但架構上第一天就預留位置。

- **Learning Trajectory**：對某個主題的瀏覽是在入門探索、工具比較、還是已經進入實作？
- **Burst Detection**：短期內某個主題的訪問量突然暴增。
- **Curiosity Graph**：概念跳轉圖 — 哪些概念是你的「橋樑節點」？
- **Rediscovery Pain**：明明以前找過的東西，又得重新搜一遍。
- **Session Archetypes**：session 分成 learn、debug、compare、buy、monitor、entertain。
- **Faceted Profile**：你是什麼樣的資訊使用者？docs-first 還是 forum-first？
- **Narrative Arc**：最近一段時間的瀏覽濃縮成一段敘事。

### 約束

- 不做人格評判型分析。
- 不對健康/政治/宗教/性向等敏感維度做推斷。
- 背景分析不能讓 app 變慢或風扇狂轉。Embedding 增量處理，LLM 按需觸發。

---

## 5. AI Provider 配置

**作為**用戶，**我想要**靈活配置 AI 模型的來源，**以便**用 Ollama、LM Studio、或雲端 API 來驅動 embedding 和 LLM 功能。

### 需求要點

- 概念模型：
  - **請求格式**（Request Format）= API 協議，如 OpenAI-compatible, Anthropic, Google
  - **Provider** = 請求格式 + Base URL + API Key + 可用模型列表 + 模型配置
  - 用戶可以創建多個 Provider
  - Provider 可以被啟用/禁用
- 預設 Provider preset：Ollama, LM Studio, OpenAI, Anthropic, Google
- 所有 preset 都支援自定義 Base URL
- Embedding 和 LLM 分別配置：兩者可以用不同的 Provider 和模型。
- **底層使用 rig.rs** 作為統一的 AI 框架，處理所有 provider 的請求格式和通信。
- day-one provider matrix：
  - chat / assistant：OpenAI-compatible、Anthropic、Google、Ollama、LM Studio
  - embeddings：OpenAI-compatible、Google、Ollama、LM Studio
  - Anthropic 在 day one 只作 chat provider，不作 embedding provider
- provider connection test 必須回傳 latency、capability report、error code、action hint、retry hint，而不是只有 pass / fail。
- secret clear 只清除 credential，不刪除 provider preset / model selection，讓使用者能先保留配置再補 key。

---

## 6. AI 計算任務系統（Job Queue）

**作為**用戶，**我想要** AI 相關的計算在背景自動運行，不阻塞我的操作，並且能控制它的行為。

### 任務生命週期

- **任務產生**：備份完成後、導入完成後、用戶手動觸發、定時掃描發現未處理記錄。
- **任務排隊**：任務加入隊列，按優先級和產生時間排序。
- **執行**：在背景異步執行，不阻塞 UI。
- **成功**：結果寫入對應的表，標記任務完成。
- **失敗**：記錄錯誤原因，按可配置的策略自動重試（最多 N 次，指數退避）。
- **暫停**：用戶可以隨時暫停所有計算任務，之後恢復。
- queue persistence 存在 canonical archive 的 SQLite `ai_jobs` 表；sidecar 只保存可重建的 embedding / vector 資產，不承擔 job state。
- queue state 精確包含 `queued`、`running`、`succeeded`、`failed`、`paused`、`cancelled`、`stale`。
- manual replay 只允許 `failed` / `cancelled` / `paused` / `stale` job；`running` job 在當前 worker 不支援 mid-flight cancel。

### 任務類型

- **Embedding 計算**：對新的或未處理的歷史紀錄生成 embedding vector（存入 LanceDB sidecar）。
- **Enrichment refetch**：背景抓取頁面內容做內容增強。
- **Insight 計算**：計算各洞察模塊的結果。
- **LLM 摘要生成**：生成 topic 命名、對比式摘要、定期總結等。

### 用戶控制

- 在 UI 中可以看到：
  - 當前隊列中有多少待處理任務
  - 正在運行的任務和進度
  - 最近完成/失敗的任務
- 可以調整**同時執行的併發任務數量**。
- 可以一鍵暫停 / 恢復所有計算。
- 可以手動觸發「掃描數據庫，把所有需要處理但還沒處理的記錄加入隊列」。
- 可以清理失敗的任務或重新排隊。

### 設計原則

- 計算任務系統完全獨立於核心備份流程 — 備份不等待 AI 計算完成。
- 計算結果存入獨立的表 / sidecar — 即使清空所有計算結果，重跑一遍就能恢復。
- 沒有配置 AI provider 的用戶完全看不到這個系統。
- semantic index 必須支援三種明確操作：incremental catch-up、full rebuild、clear-only；這三者都要留下 run / queue trace，且不能影響 canonical archive facts。
- queue payload 必須凍結 enqueue 當下的 provider / model 選擇，避免使用者之後改設定時，同一個 queued job 漂移成不同的執行語義。
