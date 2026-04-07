# M1-UX — Explorer, Export, And Onboarding

> 讀這份文檔的時機：當你要把 M1 的核心 archive 能力真正包成可被使用者理解、操作和驗收的產品體驗。  
> 這份文檔處理的是「可信的第一版使用流程」。

---

## Source Inputs

- [../../features/archive.md](../../features/archive.md)
- [../../features/recall.md](../../features/recall.md)
- [../../design/ux-principles.md](../../design/ux-principles.md)
- [../../design/screens-and-nav.md](../../design/screens-and-nav.md)
- [../../reference-review.md](../../reference-review.md)
- [../m0-foundation/frontend-shell-and-design-system.md](../m0-foundation/frontend-shell-and-design-system.md)
- [schema-backup-and-ledger.md](schema-backup-and-ledger.md)
- [schedule-security-and-storage.md](schedule-security-and-storage.md)

---

## 本工作包要交付什麼

- Onboarding v1，讓使用者完成 storage / security / schedule / first backup 的正確起步
- Dashboard v1，能概括 archive 健康與近期狀態
- Explorer v1，能檢索、篩選、查看和匯出歷史
- Audit 頁與 run detail 的主要 UI
- Export v1 和核心 smoke / e2e 驗收

2026-04-06：以上項目透過 `WORK-M1-B` 完成落地。M1 將 Explorer 的視圖密度收斂為 list + detail pane + sort / filter deep-link；timeline / alternate density view 與 rollback confirmation 延後到 M2 trust surface。

---

## WBS

### Onboarding

- [x] `M1-UX-OB-001` 根據新 prototype 和產品定位重寫 onboarding 文案，不再沿用舊 setup shell 文案。
- [x] `M1-UX-OB-002` 設計 onboarding steps：welcome、storage choice、browser detection、security choice、schedule preview、first backup ready。
- [x] `M1-UX-OB-003` 實作 browser detection summary，清楚顯示已找到哪些 browser / profile、哪些需要額外權限。
- [x] `M1-UX-OB-004` 實作 security choice 和 schedule choice 的 preview 說明，不在 onboarding 內偷做高風險操作。
- [x] `M1-UX-OB-005` 實作 first backup CTA 和成功後轉入 dashboard 的體驗，保留「稍後再做」但需清楚標示風險。
- [x] `M1-UX-OB-006` 為 onboarding 建立 empty / error / validation path，例如 snapshot unavailable、未選 profile、密碼不一致、初始化失敗。

### Dashboard V1

- [x] `M1-UX-DB-001` 實作 dashboard hero / stats 區塊，展示 archive health、last backup、profile boundary、storage 與 next action。
- [x] `M1-UX-DB-002` 實作 recent runs 模組，支援跳轉到 Audit run detail。
- [x] `M1-UX-DB-003` 實作 archive coverage / browser coverage / storage summary 模組。
- [x] `M1-UX-DB-004` 為 dashboard 建立 zero-state 和 unhealthy-state，例如尚未備份、archive 損壞警示。
- [x] `M1-UX-DB-005` 為 dashboard 定義哪些卡片屬於 M1，哪些 AI / insight 卡片要延後到 M3 / M4。

### Explorer V1

- [x] `M1-UX-EX-001` 實作 Explorer 全域搜尋欄位，支援 keyword 搜尋和基礎語法提示。
- [x] `M1-UX-EX-002` 實作 facet / filter bar，至少包含 browser、profile、domain、date range、content type。
- [x] `M1-UX-EX-003` 實作結果列表和 detail pane 的雙欄互動，確保 URL / title / time / source context 易讀。
- [x] `M1-UX-EX-004` 確認 M1 先採 list + detail pane + sort / filter deep-link 的資訊密度；timeline / alternate density view 延後到 M2 之後。
- [x] `M1-UX-EX-005` 為結果明確標示 evidence source：visit、download、search term、imported record、browser / profile。
- [x] `M1-UX-EX-006` 實作 Explorer 的 no-result、loading、archive locked 等狀態。
- [x] `M1-UX-EX-007` 為常用查詢保留 recent filter slot，即使完整 saved search 管理功能延後。

### Export And Audit UI

- [x] `M1-UX-AU-001` 實作 Audit 列表頁，支援 run 狀態、time、artifact count、warning count 顯示。
- [x] `M1-UX-AU-002` 實作 run detail 頁，顯示 summary、profile scope、artifacts、warnings、open / copy path 與 snapshot / manifest 入口。
- [x] `M1-UX-AU-003` 為 preview / manual / execute 操作設計統一的 artifact 展示樣式，避免每頁用不同視覺語言。
- [x] `M1-UX-EP-001` 實作 Export v1，支援從 Explorer 匯出當前查詢結果，提供 HTML / Markdown / Text / JSONL day-one format。
- [x] `M1-UX-EP-002` 為 export 建立欄位說明、資料範圍說明和完成後 artifact 入口。
- [x] `M1-UX-EP-003` 為 export 建立安全限制和提示，避免使用者誤以為匯出會包含不可見或已回滾資料。

### UX Polish And Trust Copy

- [x] `M1-UX-TP-001` 重寫 M1 範圍內所有 trust-critical 文案，包含備份、加密、排程、匯出、警告、空狀態。
- [x] `M1-UX-TP-002` 定義 M1 高風險按鈕的 secondary explanation，特別是 first backup、schedule preview、export boundary、audit artifact review；rekey / snapshot restore 專屬 flow 延後到後續 block。
- [x] `M1-UX-TP-003` 對照 [../../reference-review.md](../../reference-review.md) 和 prototype，確保畫面不會落回工具味過重或資訊層級混亂的舊 UI。

### Testing And Acceptance

- [x] `M1-UX-QA-001` 建立 onboarding smoke test：首次打開、偵測 browser、建立 archive、跳到 dashboard。
- [x] `M1-UX-QA-002` 建立 dashboard smoke test：顯示 recent run、跳轉 audit、處理零資料狀態。
- [x] `M1-UX-QA-003` 建立 explorer interaction test：搜尋、篩選、查看 detail、匯出。
- [x] `M1-UX-QA-004` 建立 audit interaction test：查看 run detail、artifact list、copy path。
- [x] `M1-UX-QA-005` 完成一輪 prototype 對照驗收與 browser-preview walkthrough，並把 deep-link / PME grammar / deferred Explorer density 決策回寫到 source docs。

---

## Exit Artifacts

- Onboarding v1
- Dashboard v1
- Explorer v1
- Audit UI v1
- Export v1
