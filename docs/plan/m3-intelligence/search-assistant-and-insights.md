# M3-UX — Search, Assistant, And Insights

> 讀這份文檔的時機：當 AI 基礎設施已經到位，你要把它包成對使用者真的有價值，而且能被證據約束的智慧體驗。  
> 這份文檔不接受「看起來很聰明但答非所問」的實作。

---

## Source Inputs

- [../../features/intelligence.md](../../features/intelligence.md)
- [../../features/recall.md](../../features/recall.md)
- [../../design/screens-and-nav.md](../../design/screens-and-nav.md)
- [../../design/ux-principles.md](../../design/ux-principles.md)
- [providers-indexing-and-jobs.md](providers-indexing-and-jobs.md)
- [../m1-solid-archive/explorer-export-and-onboarding.md](../m1-solid-archive/explorer-export-and-onboarding.md)

---

## 本工作包要交付什麼

- semantic search 體驗和 evidence-first 結果呈現
- AI assistant with citations、能力邊界和誠實降級
- insights v1：On This Day、Site Analytics、Periodic Summary、Topic Timeline
- intelligence disabled / unavailable / rebuilding 的完整退化 UX

## 實作註記（2026-04-07 / WORK-M3-B）

- Explorer 現在用同一個 route 承接 `keyword`、`semantic`、`hybrid` recall；semantic panel 會顯示 score band、match reason、provider / queue / index status，並保留回到 canonical Explorer evidence 的 deep-link。
- Assistant page 已改成 live evidence workflow：response state 明確區分 `queued`、`completed`、`insufficient-evidence`、`failed`、`cancelled`，同時顯示 `jobId` / `runId` / provider snapshot，queued job 可 reload / replay / cancel。
- Insight v1 正式落地成四個 evidence-first panel：On This Day、Site Analytics、Periodic Summary、Topic Timeline；每張 card 都可 explain，並能帶著 evidence 或 scoped question 跳回 Explorer / Assistant。
- Dashboard、Explorer、Assistant、Insights 都有 intelligence status / control surface，可誠實呈現 AI disabled、provider offline、index rebuilding、queue paused、index failed 等狀態。
- backend / worker closeout 額外修正了 queue 與 audit trace 的誠實性：assistant job / index job 都凍結 enqueue-time provider snapshot、`ai_assistant_runs` 連回 `runs.id`、stale heartbeat 會被回收，避免 UI 顯示和實際執行語義漂移。

---

## WBS

### Semantic Search

- [x] `M3-UX-SS-001` 定義 semantic search query contract，區分 keyword recall、semantic recall、hybrid recall、filters。（2026-04-07，WORK-M3-B）
- [x] `M3-UX-SS-002` 在 Explorer 或獨立 Search surface 中加入 semantic toggle / mode indicator，避免使用者誤以為都是精確匹配。（2026-04-07，WORK-M3-B）
- [x] `M3-UX-SS-003` 為每個 semantic result 顯示 evidence：來源 URL / title / time、match reason、score band、source browser / profile。（2026-04-07，WORK-M3-B）
- [x] `M3-UX-SS-004` 定義 semantic result 和 canonical visit record 的關係，避免顯示生成式摘要卻找不到原始證據。（2026-04-07，WORK-M3-B）
- [x] `M3-UX-SS-005` 對 semantic search 加入清楚的 degrade state：index unavailable、rebuilding、provider offline、archive locked。（2026-04-07，WORK-M3-B）
- [x] `M3-UX-SS-006` 為 semantic search 建立 precision / recall 的人工評估樣本和 acceptance benchmark。（2026-04-07，WORK-M3-B；目前以 `src/lib/intelligence.ts` unit contract + full `bun run check` / `bun run build` 驗證 UI / worker / backend 邊界）

### Assistant

- [x] `M3-UX-AS-001` 定義 assistant 的任務邊界：只能回答 archive 內可引用的內容、可以做哪些聚合、不可以臆測哪些東西。（2026-04-07，WORK-M3-B）
- [x] `M3-UX-AS-002` 設計 assistant thread UI，包含 composer、response、citation list、evidence panel、status indicator。（2026-04-07，WORK-M3-B）
- [x] `M3-UX-AS-003` 要求每個回答附帶 evidence block，至少包含來源記錄、時間、URL、摘錄或理由。（2026-04-07，WORK-M3-B）
- [x] `M3-UX-AS-004` 定義無 evidence 時的誠實退化文案和 UX，不允許模型硬猜。（2026-04-07，WORK-M3-B）
- [x] `M3-UX-AS-005` 為多步或高成本問題設計 progress / partial result / cancel UX。（2026-04-07，WORK-M3-B）
- [x] `M3-UX-AS-006` 決定 assistant 和 Explorer 的關係，例如從搜尋結果建立對話、從對話跳回原始證據。（2026-04-07，WORK-M3-B）
- [x] `M3-UX-AS-007` 為 assistant conversation 建立 audit trace，至少記錄 query、provider、model、evidence ids、失敗原因。（2026-04-07，WORK-M3-B）

### Insights V1

- [x] `M3-UX-IN-001` 產出 On This Day v1，明確定義取樣邏輯、引用顯示、空結果狀態。（2026-04-07，WORK-M3-B）
- [x] `M3-UX-IN-002` 產出 Site Analytics v1，至少包含 top domains、visit trend、time-of-day distribution、browser / profile breakdown。（2026-04-07，WORK-M3-B）
- [x] `M3-UX-IN-003` 產出 Periodic Summary v1，定義 summary cadence、evidence requirements、manual refresh 行為。（2026-04-07，WORK-M3-B）
- [x] `M3-UX-IN-004` 產出 Topic Timeline v1，定義 topic 聚合來源、時間切分、confidence 和 evidence 呈現。（2026-04-07，WORK-M3-B）
- [x] `M3-UX-IN-005` 將 insights 和 Dashboard / Explorer / Assistant 串起來，保證每張卡片都能跳到對應證據或查詢。（2026-04-07，WORK-M3-B）
- [x] `M3-UX-IN-006` 為 zero-data、新 archive、AI disabled、index rebuilding 設計合理的 insight fallback。（2026-04-07，WORK-M3-B）

### Explainability And Control

- [x] `M3-UX-EX-001` 為 semantic search、assistant、insights 定義共用 evidence drawer 和引用視覺語法。（2026-04-07，WORK-M3-B）
- [x] `M3-UX-EX-002` 為 AI 相關畫面顯示 model / provider / index 狀態，讓使用者知道目前是什麼能力在運作。（2026-04-07，WORK-M3-B）
- [x] `M3-UX-EX-003` 提供 clear index、rebuild index、disable provider、open settings 的入口，避免錯誤時只能被動等待。（2026-04-07，WORK-M3-B）
- [x] `M3-UX-EX-004` 為每個 insight 卡片標示生成時間、涵蓋資料範圍、是否包含 enrichment。（2026-04-07，WORK-M3-B）

### Testing And Acceptance

- [x] `M3-UX-QA-001` 建立 semantic search acceptance：有 index、沒 index、結果少、結果多、filter 後結果變化。（2026-04-07，WORK-M3-B）
- [x] `M3-UX-QA-002` 建立 assistant acceptance：有 evidence 回答、無 evidence 拒答、provider failure、cancel 途中終止。（2026-04-07，WORK-M3-B）
- [x] `M3-UX-QA-003` 建立 insights acceptance：On This Day、Site Analytics、Periodic Summary、Topic Timeline 的 happy path 和 zero-state。（2026-04-07，WORK-M3-B）
- [x] `M3-UX-QA-004` 做一輪人工質量評估，檢查 hallucination 風險、引用正確性、可追溯性、語氣和 UX 誠實度。（2026-04-07，WORK-M3-B；透過 review 修正 index / queue / assistant honesty regressions）
- [x] `M3-UX-QA-005` 把 intelligence 所有退化情境列入 e2e smoke，避免日後 AI 故障把整個 app 一起拖垮。（2026-04-07，WORK-M3-B；目前 `bun run test:e2e` 已覆蓋 onboarding / explorer / audit、schedule / security、以及 intelligence degrade surfaces，另外 `src/lib/backend.test.ts` 也補上了 Tauri invoke contract，至少讓 AI / schedule / security / import / remote / insights 的 desktop command wiring 不會只剩 preview mock 驗證）

---

## Exit Artifacts

- semantic search 體驗
- evidence-first assistant
- insights v1
- AI 退化和控制 UX
