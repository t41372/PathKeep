# INTELLIGENCE — 洞察

> 從 [vision-and-requirements.md](../vision-and-requirements.md) 抽出。  
> Intelligence 是建立在 Archive 夠好的基礎上的增值層。  
> 所有 AI 功能預設關閉，可以在設定中開啟。
>
> **2026-04-15 truth note:** deterministic baseline 已由 [core-intelligence-ultimate-design.md](core-intelligence-ultimate-design.md) 正式接管；[deterministic-intelligence.md](deterministic-intelligence.md) 與 [ADR-006](../architecture/decisions/006-deterministic-intelligence-boundary.md) 保留為它的歷史與 trade-off 背景。這份文檔現在只應描述 optional AI / assistant / MCP / semantic runtime / queue review 的 shipping contract；任何殘留的 deterministic insights / session / dwell / embedding-first baseline wording，都屬 legacy 敘述，不再是 accepted source of truth。
>
> **2026-04-10 packaging note:** default desktop install 仍內建 optional AI / assistant / MCP / semantic runtime；`optional` 指 capability 預設關閉、需明確設定 / provider 才會啟用，不代表第一次使用時另裝 helper 或外掛 binary。相關 shipping boundary 見 [ADR-009](../architecture/decisions/009-default-desktop-optional-intelligence-shipping.md)。
>
> **2026-04-13 current-state note:** 如果你現在要重新盤點 repo 裡 intelligence 的真實 shipped surface、前後端實作狀態、以及哪些設計文檔已經混入 legacy 描述，請先讀 [intelligence-current-state.md](intelligence-current-state.md)。它是目前給設計師與產品盤點用的白話總表。

---

## 1. 語義搜尋（Semantic Search）

**作為**用戶，**我想要**用自然語言搜尋我的歷史紀錄，**以便**能找到「我記得看過某個講 local-first 的文章但記不得具體名字」這類模糊記憶。

### 需求要點

- 基於 embedding 的向量相似度搜尋，使用 rig.rs 驅動 embedding pipeline，LanceDB 作為向量存儲和 ANN 索引。
- Embedding 增量計算、本地索引、避免重算。
- 搜尋不只找頁面，還要支持找 session、task 和 topic level 的語義匹配。
- day-one recall mode 明確區分 `keyword`、`semantic`、`hybrid`；semantic / hybrid 必須顯示目前使用的 provider / model / index state，語義檢索不可用時要明講退化成 keyword recall。
- v1 semantic result 以 canonical visit evidence 為核心：至少回傳 `historyId`、profile / browser、URL / title、visited time、match reason、score band，並能 deep-link 回 Explorer 查原始記錄。
- semantic index state 至少要能誠實區分 `disabled`、`blocked`、`empty`、`queued`、`paused`、`rebuilding`、`failed`、`stale`、`ready`、`degraded`。`stale` 代表 archive visibility / import watermark 或 approved enrichment 已改變，使用者必須明確 rebuild，而不是假裝 index 仍是最新。
- runtime contract：semantic retrieval 必須先查 LanceDB sidecar；若 sidecar 缺失、過期或失敗，PathKeep 只能誠實退回 lexical recall，不得在請求路徑做全庫向量掃描。semantic metadata / queue / assistant trace 固定落在 `derived/history-intelligence.sqlite`；SQLite 不再承擔向量 payload mirror。

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
- queued assistant request 可在執行前 replay / cancel；running AI / deterministic job 改為 cooperative stop request，而不是假裝立即中斷。UI 必須清楚說明「已請求取消，會在目前 phase / chunk 邊界停止」。
- Assistant 必須尊重 shell 的共享 profile scope；若使用者透過 deep-link 帶進明確 `profileId`，頁面級 scope 優先於共享 scope。
- Assistant 的 empty / AI-disabled state 不能只剩靜態說明；至少要提供 seeded prompt 建議、queue / settings 修復入口，以及在共享 profile scope 生效時明講目前回答邊界是 scoped view。

---

## 3. 外部 AI 工具整合

**作為**用戶，**我想要**能讓外部 AI 工具存取我的瀏覽歷史，**以便**在其他 AI 助手中也能搜尋和利用我的歷史紀錄。

### MCP Server

- 在 Settings 的 AI provider / integration preferences 中手動開啟。
- App 只會在 AI / MCP 明確啟用、且當前 app session 處於 unlocked 時啟動本地 MCP server。
- 提供搜尋、檢索歷史紀錄的 MCP tools；若 session 之後被 App Lock 鎖住，history query 相關 tool 必須回傳 locked refusal，而不是繞過 UI 直接讀 archive。
- 安全考量：只綁定 localhost，不對外暴露，並且必須尊重 visibility、lock state 與 capability gating。
- 若沒有 embedding provider，但使用者仍明確啟用 AI + MCP，read-only recall 仍可在 lexical mode 下運作；consent / capability copy 必須明講 semantic recall 目前不可用。
- Integrations 的 generated-artifact review 是 preview-only surface：要顯示 command、MCP JSON、skill markdown、consent summary、scope boundary 與 audit trace，供使用者手動複製；不能假裝已經自動安裝到外部工具。Settings 只保留 provider / API key / capability preferences。
- 每一次 MCP search 都必須寫入 dedicated `mcp_query` run，而不是混進 backup / assistant / generic utility run。

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
  URL 結構、domain / page 類型、搜尋關鍵詞、transition/referrer、
  訪問時間/星期/時段、訪問頻率、reopen / revisit evidence tier

層 2：Burst / Query Group / Thread 構建
  把零散的頁面訪問聚合成有意義的單元：
  相鄰訪問 → burst → query group → cross-burst / cross-day thread merge → 偵測 reopen

層 3：Topic 聚類和時間序列
  deterministic topic / reference-page / source-effectiveness signals →
  optional embeddings 只作 additive coverage，而不是 baseline truth

層 4：LLM 增強
  為 topic 和 thread 起人類可讀的名字 →
  生成對比式摘要 → 解釋為什麼某個洞察值得關注
```

### V1 洞察功能

##### 🕐 歷史上的今天（On This Day）

讓用戶看到過去幾年的同一天，自己在瀏覽什麼。

- 拉出歷年同一天（±1 天容差）的歷史紀錄，按年份分組展示。
- 同一天的比對以使用者目前系統 timezone 的本地日曆日為準，不用 raw UTC 日期切片假裝對齊「今天」。
- 當前年份今天的紀錄不屬於 `On This Day`；這張卡只用來回看過去幾年的同一天，不能把今天剛產生的瀏覽紀錄混進歷史回顧。
- 如果有足夠數據，用 LLM 生成一句話摘要。
- 這張卡現在是 **Dashboard-only** surface，不再放進 `/intelligence`；因為它不受 Intelligence route time scope 影響，留在 Dashboard 更誠實。
- **實現**：純數據庫查詢，不需要 embedding。有 LLM 時可生成摘要，沒有也能用。

##### 📊 定期總結（Periodic Summaries）

自動生成日度、週度、月度、年度的瀏覽總結。

- **日度總結**：今天你主要在研究什麼？訪問了多少個頁面？最活躍的 domain 是什麼？
- **週度總結**：和上週相比，你的研究重心有什麼變化？本週新出現了什麼主題？
- **月度總結**：本月的主題分布、最深入的研究線、最常用的資訊來源。
- **年度總結**：年度回顧 — 你這一年的注意力分布、主要的研究階段和轉折點。
- **實現**：統計部分不需要 AI。2026-04-12 起，backend deterministic registry 必須至少輸出 backend-owned periodic summary 與 adjacent-window contrastive summary，讓 AI disabled 時仍可顯示 summary card；LLM 只負責之後把這些結構化差異改寫成人話。

##### 🌊 Topic Timeline（主題時間軸）

你最近在關注什麼？哪些主題在升溫？哪些在冷卻？

- 以視覺化的方式展示主題隨時間的變化。
- 用戶可以點擊任何一個主題，看到具體頁面和時間分布。
- **2026-04-13 truth note:** 現在 shipping 的 topic timeline 已不是這段最早寫下的 embedding-first 方案。當前實作是 deterministic aggregation：先由 burst / query group / thread 建立研究線，再用 thread / query-group title 的 token similarity 收斂 topic label，UI 目前呈現的是 lightweight topic overview，不是完整 semantic topic workspace。embedding 與 LLM naming 只保留為 optional / future additive layer。

##### 🧵 Task / Thread Detection（任務和研究線偵測）

自動把歷史紀錄切成一個個有意義的任務。

- 自動偵測進行中的研究線：跨天持續的、語義上連貫的一系列訪問。
- 偵測「任務重新打開」：停了幾天又回來繼續的研究線。
- **實現**：M5 之後以 burst / query group / reference-page reuse / reopen evidence 為 baseline；embedding merge 只作 optional additive layer。

##### 🔄 Open Loops（未完成的任務）

找出反覆在看但一直沒完成的事情。

- **實現**：基於 thread 的 revisit 次數和是否出現收斂信號。

##### 💎 Important but Unsaved（重要但沒保存的頁面）

你看了很多次、顯然覺得重要，但從來沒有 bookmark 的頁面。

- **實現**：importance 以 revisit、reference-page reuse、query-group 後段穩定落點與 deterministic centrality signal 為主；不再依賴 estimated dwell。
- **2026-04-12 truth note:** 這張卡仍 deferred。當前 canonical archive 尚未 ingest bookmark / saved-page facts，所以 backend 不得假裝已能判定「沒保存」；在 bookmark source 落地前，只允許保留為 source-doc deferred item，不得在 UI 冒充 shipping surface。

##### 📈 Explore vs Exploit（探索 vs 深挖）

你最近是在到處看新東西，還是在集中鑽研某個問題？

- **實現**：計算 domain Shannon entropy、新 domain 佔比、revisit concentration。

##### 🗺️ Source Role Map（資訊來源角色圖）

根據實際使用方式，把常用網站分成角色：搜尋入口、社群探索、問題定位、學習消費等。

##### 🔍 Query Reformulation Ladder（搜尋問題的演化路徑）

分析同一研究線中搜尋關鍵詞的演化方向。目前只有 Chromium 系瀏覽器能提供這個數據。

##### 🌐 Site Analytics（網站統計）

按 domain 統計訪問次數、revisit / reopen evidence 與 source role。純數據庫查詢和統計計算，不需要 AI。

##### 🎯 Contrastive Summary（對比式摘要）

「這週 vs 上週，你的研究重心變了什麼？」把結構化差異交給 LLM 寫成人話。

- **2026-04-12 implementation note:** deterministic backend 現在必須先輸出 adjacent-window contrast summary，再讓 optional LLM 在此之上改寫；沒有 AI provider 時，Insights 仍要顯示 deterministic contrast card，而不是整塊 summary 變空。

### V1 洞察顯示契約

- 每張 insight card 都必須顯示生成時間、資料視窗、evidence 數量，以及是否依賴 Chromium-only enhancement。
- Dashboard 的 On This Day，以及 Intelligence 的 Site Analytics / Periodic Summary / Topic Timeline，都必須能 deep-link 回 Explorer evidence，或帶著 scoped question 跳進 Assistant。
- deterministic intelligence baseline 也屬於正式 shipping surface：`open-loop` / `revisit` cards 與 `query ladders` 必須在沒有 embedding / LLM 時仍可用；`query ladders` 只在 Chromium search term evidence 形成至少 2-step refinement 時顯示，並能 deep-link 回 Explorer 的 canonical query。
- 即使 AI disabled、provider unavailable、embedding 尚未建立，或 AI-generated card / topic surface 暫時為空，On This Day（Dashboard）、Site Analytics、Periodic Summary 這類 canonical / statistical surface 仍必須用純資料庫 / 統計結果繼續顯示，而不是讓整個 intelligence 區塊變空白。
- explainability panel 必須可列出該 insight 使用的 evidence 與補充 notes，不能只顯示一段摘要。
- 切換共享 profile scope、重新整理 insights，或 explain request 失敗時，Insights 必須先清空上一個 scope / 上一次 explain 的 cards、selected insight、explanation 與相關 error，再等待新的結果；不能把舊 evidence 殘留在新的 scoped view。
- zero-data、新 archive、AI disabled、index rebuilding、provider unavailable 等情境都必須回傳 honest fallback，而不是合成看似完整的 insight。
- Insights page 必須顯示目前是否套用了共享 profile scope；若有套用，UI 要明講這是 scoped view，而不是假裝所有 KPI 都已 per-profile 重算。

### M4-A 進階 intelligence slice

- Insights 頁現在還必須顯示 storage analytics：tracked storage、reclaimable bytes、dominant group，以及 latest growth signal。這個 growth signal 必須能 deep-link 回對應的 Audit run，而不是只停在摘要數字。
- storage analytics 的 top-level summary 現在固定先分成兩個 bucket：`core history`（canonical archive + source evidence）與 `other data`（search / intelligence projection、semantic index、content blobs、audit artifacts、exports、temporary files）。更細的 breakdown 再在卡片內展開，而不是先讓使用者背四個內部 slice 名稱。
- Maintenance 頁必須提供 enrichment / derived-state panel，顯示 built-in plugin registry、network boundary、freshness、derived tables、storage impact、enable / disable control，以及 rebuild / clear controls。runtime queue / recent job review 不在 Maintenance 內複製，必須 deep-link 到 Jobs。plugin / module 的內部版本標記屬 diagnostics / runtime trace，不應佔據主產品 review chrome。
- shell chrome 左下角必須常駐一個小型 background-work status strip，顯示 queued / running / failed 概況並 deep-link 到 dedicated Jobs 頁；使用者不應該只能靠 Settings / Intelligence 才知道 background queue 還在跑什麼。
- `/intelligence` route entry 不得再一次 fan-out 二十多條 foreground IPC request。accepted shipping contract 是 route-level staged overview：先批次載入 runtime digest、digest summary、首屏可見卡片與其 section metadata，再在 first paint / idle 後補 secondary grid 與較低優先 detail。
- sidebar、Dashboard 與 `/intelligence` 的 runtime digest 必須共享同一份 shell-level runtime polling source；不能讓 `loadAiQueueStatus` / `loadIntelligenceRuntime` 因多個 route/surface 同時掛載而被重複輪詢。
- long-running derived-data job 不能只顯示抽象的 `running`。deterministic rebuild 至少要持續更新 phase、heartbeat 與 coarse progress（例如目前在哪個 phase、已處理幾筆 / 總筆數），讓 Jobs 頁和 shell footer 都能分辨「仍在前進」與「疑似卡死」。
- Jobs 頁的 primary UX contract 不是把所有 queue / plugin / module 平鋪出來，而是先讓使用者分清楚 `running now`、`queued / deferred`、`needs review` 三件事。特別是 `readable-content-refetch` 的大量 queued work 必須先明講這是為了讓 deterministic rebuild 先完成，而不是用 layout 讓人誤以為「所有網頁內容抓取都失敗」。
- `readable-content-refetch` 的 failure surface 必須先回到人話：像 `PDF / JSON / sign-in redirect / rate-limit` 這類常見邊界，要比 raw `unsupported-content` 或抽象 status 更先被使用者看到。raw status / runtime trace 仍可保留在 support 層，但不應當是主 review copy。
- M5-A 起正式 shipping 的 built-in enrichment plugin 有兩個：`title-normalization`（local-only，版本 `m5-v1`）與 `readable-content-refetch`（network-backed，版本 `m4-v1`）。兩者都屬 derived-state runtime，不可改寫 canonical archive facts。
- `title-normalization` 預設啟用，負責把 noisy browser title、redirect suffix 與 URL fallback 收斂成更穩定的 evidence label。停用後，deterministic insights 仍可用，但必須誠實回退到 raw title / URL structural signals。
- `readable-content-refetch` 預設啟用、freshness window 7 天，也承載第一批 built-in site adapters：影片頁面（YouTube / Vimeo）可優先提取 title、channel / author、duration、publish date 與 description，避免把 noisy page chrome 誤當成主要 evidence。
- built-in enrichment runtime 目前仍是 first-party only：Maintenance / Jobs / Intelligence 可以 review 內建 runtime state，retry / cancel 的 canonical runtime queue surface 是 Jobs；third-party plugin execution 仍 deferred，直到獨立 sandbox / permission ADR 存在。
- queue / runtime contract 以 durable lease + heartbeat + cooperative stop 為準：claim 必須 compare-and-set，running cancel 只會設 stop request，worker 需在 phase / chunk 邊界自行結束並留下 cancelled trace；terminal success 不得覆蓋已 cancel / failed 的 job。
- derived intelligence refresh 在 backup / import 成功後必須自動排入 runtime job 並留下可 review 的 queue / recent-job trace；Insights / Settings 仍保留手動 rebuild 作為 override，但不能再把最新 derived state 完全變成使用者自己記得去按的 follow-up。
- 2026-04-15 之後，deterministic/Core Intelligence 的主路徑改為直接讀 `sessions`、`search_trails`、`query_families`、rollups、`refind_pages`、`source_effectiveness`、`reopened_investigations` 等 persisted entities / rollups，而不是再靠 `load_insights()` 這種整包 snapshot-first read model。若 repo 內仍保留舊 snapshot payload fallback，只能視為 legacy inert path，不再是 accepted shipping contract。
- Insights 頁自己的頂部 runtime surface 只能是 digest，而不是第二個 Jobs。它應該先顯示 analysis snapshot，再把 runtime queue 收斂成一個小型摘要與 `Open Jobs` 入口，避免整頁一打開就被 runtime / retry / cancel chrome 吞掉主洞察內容。
- Insights 的內容分組必須符合 `spotlight -> research signals -> evidence / health`：最先看到的是 highlights / summary / top sites / browsing rhythm，接著才是 query groups、topic timeline、query evolution，最後才是 reference pages、source effectiveness、storage analytics 與 deterministic module health。`On This Day` 留在 Dashboard，不再佔據 `/intelligence` 的 spotlight 區。
- clear derived state 必須回傳按 stage 分組的清除數量報告：至少涵蓋 `visit-derived-facts` rows、daily rollup rows、structural rows、runtime rows；只有在該維護路徑真的清 enrichment 時才額外回 enrichment rows。報告也必須明講 canonical archive、manifests、rollback state 完全未被動到。
- full rebuild 不得先把 live derived snapshot 清空再等待後續重算完成；scope 內的 derived rows、snapshot payload 與 deterministic module runtime 狀態更新必須以同一個 archive transaction 原子替換，避免只清掉一半或留下不一致的 stale trace。這一輪 rebuild 仍必須留下 run-linked report 和 notes，避免 advanced intelligence 變成不可追蹤的黑盒。
- queue / progress persistence 也屬 recoverability contract：如果使用者突然關閉 app、程序崩潰或主機斷電，重新開啟後 Jobs 頁必須能誠實呈現上次停在哪個 job、是否已被 recover/requeue，以及最後一次 heartbeat / progress update；不可把 interrupted long-running work 假裝成從未發生。
- source-effectiveness / reference-page 類 surface 的 domain key 必須跟 canonical visit evidence 使用同一套 registrable-domain normalization；不能因為 `docs.example.com` / `www.example.com` 分裂而把同一來源錯拆成多個 source role。
- Dashboard 的 aggregate archive KPIs 仍以 archive-wide read model 為準；共享 profile scope 目前只保證影響 insight fetch、assistant retrieval 與 Explorer 預設 filter，不能誤寫成所有 dashboard 指標都已 profile-partitioned。
- Dashboard 的 `Browsing Rhythm` preview 固定以 calendar year 呈現，若 archive 內橫跨多個年份可切換年份；年份來源來自 `getDiscoveryTrend(..., 'day')` 的 `availableYears`，而不是把 hourly detail API 誤當成年視圖的 source of truth。year pager 必須遵守時間直覺：左箭頭看更早的年份、右箭頭看較新的年份；若選中的年份只覆蓋部分日期，卡片還要直接顯示該年份內實際有資料的起訖範圍。
- `Browsing Rhythm` 初次進頁時只顯示日曆熱力圖 shell；不得在 first paint 自動抓同日 digest / top sites / hourly detail。點日格後的 primary workflow 現在是進 `/intelligence/day/:date`；若 overview 保留任何 inline preview，也只能是 secondary information。
- `/intelligence` 頂部固定提供 `Insight Access` strip：使用者可直接輸入本地日曆日或 domain，打開完整 day/domain insights route。這條 strip 必須吃 shared href grammar，而不是再長出另一套局部 state / fetch story。
- M7 起，active intelligence entity 也必須統一吃 shared entity contract：`query family`、`refind page`、`session`、`trail` 正式有 first-class shared insights route；`reopened investigation`、`habit`、`stable source`、`friction`、`multi-browser diff`、`compare set` 等 surface 也都必須解析到單一 shared destination，而不是各自拼 `/explorer` deep-link。
- `session` / `trail` 在 Explorer 仍維持 browse-first canonical grouped view；shared insights route 只承接 reusable detail / explainability / evidence CTA，不得把 grouped Explorer 改成 route-only workflow。
- M8 起，`compare set` 也正式升格成 first-class shared insights route，`path flow` 則改成 stable `flowId` + typed `steps`；compare-set / path-flow 相關的 aggregate context 必須透過 additive `focusType` / `focusId` query grammar 在 shared non-overview routes 間流動，而不是回退成 consumer-local state。
- `refind` route 直接使用 encoded canonical URL 作 path identity；shared focus contract 只允許受限的 `focusType` / `focusId`，且 overview 不承接 focus。trusted external-output payload 可帶 structured entity targets 供 app-link reuse，而 `public snapshot` 仍維持 redacted。
- route 切換時必須丟棄過期 request；離開 `/intelligence` 後，上一個 scope / date range 的 section response 不得再 commit 回 UI，也不得偷偷繼續觸發後續 detail fetch。
- 2026-04-09 truth closeout：目前的 intelligence 支援邊界與未完成項，見 [../plan/m4-full-polish/intelligence-60-year-envelope.md](../plan/m4-full-polish/intelligence-60-year-envelope.md)。在該文件有真實 large-archive artifact 之前，不可把 PathKeep 寫成已完成「60 年資料量、所有 AI 開啟、仍可流暢使用全部功能」的最終性能背書。

### Profile-Scoped Insights（Profile 級別洞察篩選）

洞察系統支援以特定瀏覽器 profile 為範圍查看洞察資料，增強現有的 shared profile scope 功能。

- 透過 topbar 的共享 profile scope 選擇特定 profile 後，Insights 頁面的可篩選 surface 自動切換為 scoped view。
- **可篩選的 surface**：insight cards、topic timeline、query groups、threads、query ladders、reference pages、source effectiveness、periodic summaries。
- **仍維持 archive-wide 的資料**：Dashboard KPIs、storage analytics、growth signal。
- Insights 頁面在 scoped 模式下必須以 callout 或 badge 明確標示「目前為 profile-scoped view，部分統計仍為 archive-wide」，避免用戶誤解。
- 切換 scope 不產生新 route，沿用 shell chrome 的 shared scope 或 query string `profileId`，保持與 Explorer / Assistant 的 scope 語法一致；若頁面 URL 已帶明確 `profileId`，它優先於 shared scope。
- 從 Insights 回 Explorer 的 drilldown，包括 Site Analytics、Topic Timeline、Reference Pages 與 explain citations，都必須保留目前 `profileId`；不能從 scoped view 悄悄掉回 archive-wide 搜尋。Dashboard 的 On This Day 也必須沿用同樣的 scope honesty，但不再屬於 `/intelligence` route grammar。
- Day Insights 現在是正式 route `/intelligence/day/:date`；它只使用本地日曆日 path + `profileId` query，並在返回 Explorer evidence 時固定帶 `start=end=<date>` exact-day window。
- Domain Deep Dive 現在是正式 route `/intelligence/domain/:domain`，user-facing IA 視為 `Domain Insights`；它必須沿用與 `/intelligence` 相同的 `range` / `start` / `end` / `profileId` query contract，讓 domain drilldown 可重整、可分享、可返回原本的 scoped overview。
- day/domain 的 primary interaction 採 `Insights first`：overview card、Dashboard、Explorer 與其他 active surface 若已經握有這兩種 entity，就應優先帶去完整 insights route；Explorer evidence 仍保留，但降為 secondary CTA。
- 導航規則 → `docs/design/screens-and-nav.md` §Profile-Scoped Insights

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
- Settings 的 provider editor 採本地 draft + explicit save：欄位編輯不應在每次輸入時直接落盤或觸發 blocking overlay；secret store / clear 仍保留獨立明確動作。
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
- queue persistence 現在存在 `derived/history-intelligence.sqlite` 的 `ai_jobs` / `intelligence_jobs`；sidecar 只保存可重建的 embedding / vector 資產，不承擔 job state。
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
- 計算結果存入獨立的 intelligence projection / sidecar — 即使清空所有計算結果，重跑一遍就能恢復。
- 沒有配置 AI provider 的用戶完全看不到這個系統。
- semantic index 必須支援三種明確操作：incremental catch-up、full rebuild、clear-only；這三者都要留下 run / queue trace，且不能影響 canonical archive facts。
- v1 invalidation contract 先以 honest stale detection + manual rebuild 落地：import / rollback / visibility change / approved enrichment freshness 改變時，UI 必須把 index state 標成 stale。是否自動 re-enqueue rebuild 屬後續 work，不可假裝 day-one 已完成。
- queue payload 必須凍結 enqueue 當下的 provider / model 選擇，避免使用者之後改設定時，同一個 queued job 漂移成不同的執行語義。
- M5-A 起的 queue/runtime surface 現在必須在 archive 解鎖且 queue 未暫停時自動背景執行：AI index job、retry/replay 後的 AI queue，以及 deterministic / enrichment runtime job 都不能再卡住前台 UI。Maintenance / Intelligence 只保留摘要或 rebuild / clear 入口，而 dedicated Jobs 頁則作為 always-on log / retry / cancel / progress / recovery surface。
- deterministic rebuild 屬於 baseline intelligence，可選 enrichment 只是在後面補更多證據。兩者同時排隊時，deterministic rebuild 必須先跑，避免使用者看到 queue 很忙，但無 AI 的 Insights 仍然完全不可用。
- automatic post-backup/import deterministic refresh 仍不得重新把 network enrichment 塞回 inline backup critical path；backup 只負責 enqueue 與啟動 background rebuild，後續 readable-content 類工作必須留在 queue 裡獨立處理。
