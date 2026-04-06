# M0-FE — Frontend Shell And Design System

> 讀這份文檔的時機：當你要重做整個桌面端 UI 骨架、對齊設計 prototype、清理舊 `AppNew` 結構時。  
> 這份文檔只管前端骨架和畫面契約，不處理底層 schema / parser 的實作細節。

---

## Source Inputs

- [../../vision-and-requirements.md](../../vision-and-requirements.md)
- [../../design/ux-principles.md](../../design/ux-principles.md)
- [../../design/screens-and-nav.md](../../design/screens-and-nav.md)
- [../../reference-review.md](../../reference-review.md)
- `reference/PathKeep — Desktop UI Design/`
- [../program/repo-baseline.md](../program/repo-baseline.md)
- [backend-and-data-rearchitecture.md](backend-and-data-rearchitecture.md)

---

## 本工作包要交付什麼

- 新的 app shell、route tree、sidebar IA、page title / breadcrumb 規範
- 從 prototype 抽取出的正式 design tokens、component primitives、theme contract
- 新的 screen skeleton：Onboarding、Dashboard、Explorer、Insights、Assistant、Import、Audit、Schedule、Security、Settings
- 舊 `AppNew` / `App.css` / 單檔巨大測試的淘汰策略
- 新前端對後端的資料契約與 loading / error / empty state 規範

---

## WBS

### IA And Route Tree

- [ ] `M0-FE-IA-001` 盤點目前 [`src/AppNew.tsx`](../../../src/AppNew.tsx) 的 page model、context model、navigation model，標記哪些概念要保留、哪些直接淘汰。
- [ ] `M0-FE-IA-002` 依照 [../../design/screens-and-nav.md](../../design/screens-and-nav.md) 凍結新的 top-level route tree，包含每個 route 的 path、title、sidebar group、secondary action。
- [ ] `M0-FE-IA-003` 定義 Dashboard / Explorer / Insights / Assistant / Import / Audit / Schedule / Security / Settings 的 sidebar hierarchy，不再沿用舊 setup-first shell。
- [ ] `M0-FE-IA-004` 定義 onboarding 何時獨立於主 shell 顯示、何時可被再次打開、完成後如何回到 dashboard。
- [ ] `M0-FE-IA-005` 定義全域 command bar / search entry 的位置、快捷鍵、與 Explorer / Assistant 的關係。
- [ ] `M0-FE-IA-006` 為每個 route 定義 URL state contract，例如 Explorer filters、Assistant thread id、Audit run id、Settings sub-tab。
- [ ] `M0-FE-IA-007` 定義 breadcrumb、page header、status rail、context panel 的共用規範，避免每頁自行發明資訊階層。
- [ ] `M0-FE-IA-008` 明確列出 prototype 未覆蓋的 route state：empty state、error state、permission denied、loading、zero data、first-run。

### Design Tokens And Primitive Layer

- [ ] `M0-FE-DS-001` 從 prototype `style.css` 抽取顏色、字級、spacing、radius、shadow、surface、border、motion token。
- [ ] `M0-FE-DS-002` 形成正式 token 文檔和程式碼落點，避免 token 繼續散落在單頁 CSS 裡。
- [ ] `M0-FE-DS-003` 決定 token 的實作方式：CSS variables、theme helper 或等價方案；要求和現有 Vite / React 結構相容。
- [ ] `M0-FE-DS-004` 凍結 light / dark theme contract，確認 day one 是否 dark-first，但不讓 light theme 完全消失。
- [ ] `M0-FE-DS-005` 為 success / warning / danger / info / muted / selected / focus 定義語義色，不允許頁面自己挑色。
- [ ] `M0-FE-DS-006` 定義資料密度階梯：sidebar、table row、timeline item、chip、code snippet、artifact card 的 spacing 規則。
- [ ] `M0-FE-DS-007` 為數據可視化建立最小 token 集：chart palette、heatmap density、empty chart fallback、trend up / down 狀態。
- [ ] `M0-FE-DS-008` 決定字體資產策略，確認 prototype 字體在桌面端的授權、打包和跨平台 fallback。
- [ ] `M0-FE-DS-009` 為動畫建立 guardrail：page reveal、panel transition、loading shimmer、背景動效允許哪些效果，哪些不要做過頭。

### Shell And Shared Layout

- [ ] `M0-FE-SH-001` 建立新的 `src/app/` 或等價目錄作為 shell 入口，停止讓 `AppNew` 承載全部畫面與狀態。
- [ ] `M0-FE-SH-002` 建立 route-aware shell 結構，分清 onboarding shell、main shell、settings shell 是否共用 chrome。
- [ ] `M0-FE-SH-003` 建立共用 sidebar、top bar、command entry、status summary、content container primitives。
- [ ] `M0-FE-SH-004` 定義 shell 層的 responsive 規則，至少覆蓋桌面窄寬度和小窗模式，不追求 mobile app，但也不能一縮就崩。
- [ ] `M0-FE-SH-005` 建立 page metadata system，讓 route 可以聲明 title、subtitle、danger level、required capability、loading policy。
- [ ] `M0-FE-SH-006` 建立 global empty / loading / error component contract，讓後續頁面可以共用而不是複製文案和布局。
- [ ] `M0-FE-SH-007` 為 permission-required flows 建立專用 panel，例如 Full Disk Access、directory permission、keyring unavailable、AI provider not configured。
- [ ] `M0-FE-SH-008` 決定全域狀態管理策略，只保留最小 shared state；不要再把整個 app 塞回一個肥大 context。

### Screen Skeletons

- [ ] `M0-FE-PG-001` 建立 Onboarding skeleton，至少含產品定位、storage choice、browser detection、schedule preview、privacy promise。
- [ ] `M0-FE-PG-002` 建立 Dashboard skeleton，預留 recent runs、archive health、next schedule、storage summary、trust callouts 插槽。
- [ ] `M0-FE-PG-003` 建立 Explorer skeleton，預留搜索框、facet bar、timeline / list switch、detail pane、export action、saved search slot。
- [ ] `M0-FE-PG-004` 建立 Insights skeleton，預留 chart modules、time range、zero-state、AI unavailable fallback。
- [ ] `M0-FE-PG-005` 建立 Assistant skeleton，預留 thread list、composer、evidence panel、capability gating、provider not configured fallback。
- [ ] `M0-FE-PG-006` 建立 Import skeleton，預留 file picker、dry-run summary、preview artifacts、quarantine status、execute action。
- [ ] `M0-FE-PG-007` 建立 Audit skeleton，預留 run ledger table、run detail、artifact viewer、copy command、rollback entrypoint。
- [ ] `M0-FE-PG-008` 建立 Schedule、Security、Settings skeleton，為 PME、encryption、providers、language、storage location 提前留位。
- [ ] `M0-FE-PG-009` 逐頁補上 prototype 沒畫但 production 必需的 empty / error / loading / offline / permission-denied states。

### Frontend Data Contract

- [ ] `M0-FE-DC-001` 為 Dashboard 定義 day-one IPC data contract，區分必需資料、lazy-loaded 資料、可選 intelligence 資料。
- [ ] `M0-FE-DC-002` 為 Explorer 定義 query result contract，包含 filters、cursor / pagination、sort、highlight、evidence placeholder。
- [ ] `M0-FE-DC-003` 為 Audit / Run detail 定義 artifact contract，包含 manifest、snapshot、warnings、copyable command、log excerpt。
- [ ] `M0-FE-DC-004` 為 Onboarding / Schedule / Security / Settings 定義 command-response contract，區分 preview mode 和 execute mode。
- [ ] `M0-FE-DC-005` 把 [`src/lib/backend.ts`](../../../src/lib/backend.ts) 裡的假資料和 IPC wrapper 拆開，建立真正的 typed bridge layer。
- [ ] `M0-FE-DC-006` 決定前端如何表示 capability gating，例如 `archive_ready`、`scheduler_supported`、`keyring_available`、`ai_configured`。

### Legacy Removal And Cutover

- [ ] `M0-FE-LG-001` 盤點 [`src/AppNew.tsx`](../../../src/AppNew.tsx)、[`src/App.css`](../../../src/App.css)、[`src/AppNew.test.tsx`](../../../src/AppNew.test.tsx)、[`src/lib/i18n.ts`](../../../src/lib/i18n.ts) 的可重用片段和應淘汰片段。
- [ ] `M0-FE-LG-002` 把舊 shell 中仍有價值的文案、型別、輔助函式搬到新結構或正式刪除，不留「先放著以後再看」。
- [ ] `M0-FE-LG-003` 將 `AppNew` 從主入口移除後，保留很短期的過渡策略，但不能讓新舊 shell 長期雙軌。
- [ ] `M0-FE-LG-004` 重寫或刪除舊 setup-first 相關 CSS 和測試斷言，避免新 shell 被舊快照和舊文案拖住。
- [ ] `M0-FE-LG-005` 盤點 `src/pages/` 舊頁面，標記 `rewrite in place`、`replace with new file`、`delete after cutover`。

### Testing And Design Verification

- [ ] `M0-FE-QA-001` 為新 shell 建立最小 smoke test：app 啟動、route 切換、sidebar 可見、onboarding gating 正常。
- [ ] `M0-FE-QA-002` 把巨型 [`src/AppNew.test.tsx`](../../../src/AppNew.test.tsx) 拆成 route-scoped test files，對應新頁面和 shared primitives。
- [ ] `M0-FE-QA-003` 建立 visual review checklist，逐頁比對 prototype 和實作，記錄可接受偏差和不可接受偏差。
- [ ] `M0-FE-QA-004` 定義 accessibility baseline：keyboard nav、focus ring、contrast、reduced motion、screen reader landmarks。
- [ ] `M0-FE-QA-005` 為 design token 建立 snapshot 或 contract test，防止顏色 / 間距 / 語義狀態被無意改壞。
- [ ] `M0-FE-QA-006` 在 M0 結束前，更新 e2e smoke 目標，讓 Playwright 不再驗證舊 setup shell。

---

## Exit Artifacts

- 新 shell 實作與 route map
- design tokens 與 primitive component layer
- prototype gap list 和補稿需求清單
- 前端 IPC contract 草案
- 舊 UI 刪除 / 保留清單與 cutover 順序
