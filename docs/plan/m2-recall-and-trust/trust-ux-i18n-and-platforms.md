# M2-TX — Trust UX, I18n, And Platforms

> 讀這份文檔的時機：當你要把「可操作」提升成「值得信任地長期使用」，並把 M1 的單平台、單語系、局部 PME 補齊成完整產品體驗。  
> 這份文檔是 Recall & Trust 的前端與平台落地層。

---

## Source Inputs

- [../../design/ux-principles.md](../../design/ux-principles.md)
- [../../design/screens-and-nav.md](../../design/screens-and-nav.md)
- [../../features/archive.md](../../features/archive.md)
- [../../features/recall.md](../../features/recall.md)
- [../program/research-and-decisions.md](../program/research-and-decisions.md)
- [../m1-solid-archive/explorer-export-and-onboarding.md](../m1-solid-archive/explorer-export-and-onboarding.md)
- [imports-browsers-and-rollback.md](imports-browsers-and-rollback.md)

---

## 本工作包要交付什麼

- PME 從 schedule 擴展到 import、rollback、rekey、doctor 等高風險操作
- Trust-first 的 Audit / Settings / warning / guidance UX
- 正式 i18n 架構和核心語系內容
- Windows / Linux 平台的排程和操作指引

> **Closeout（2026-04-07，WORK-M2-B）**：核心 trust UX、namespace-based i18n、平台 capability / troubleshooting UX 已落地；更深的 audit diff/filter、reduced motion 與更完整的 AX contract test 留待後續 polish work。

---

## WBS

### PME Interaction Grammar

- [x] `M2-TX-PM-001` 凍結 PME interaction grammar：Preview、Manual、Execute 在每種高風險操作中都要有一致語意。
- [x] `M2-TX-PM-002` 為 import 流程實作 PME UI，清楚分出 dry-run summary、manual inspection、execute import。
- [x] `M2-TX-PM-003` 為 rollback / un-revert 實作 PME UI，提供影響範圍預覽、確認、完成後驗證。
- [x] `M2-TX-PM-004` 為 rekey、snapshot restore、doctor repair 等操作補齊 PME 流程和共用 component。
- [x] `M2-TX-PM-005` 定義每個 PME 流程要顯示哪些 artifact、copyable command、manual fallback、risk explanation。

### Trust And Audit UX

- [ ] `M2-TX-AU-001` 深化 Audit UI，支援按 run type、severity、source、profile、artifact type 篩選。
- [ ] `M2-TX-AU-002` 實作 run-to-run diff 或至少 summary delta，讓使用者知道某次 import / rollback 改了什麼。
- [x] `M2-TX-AU-003` 為 warning 和 finding 定義一致視覺語法，區分 information、needs attention、danger、blocked。
- [x] `M2-TX-AU-004` 補齊 permission guidance UX，例如 Full Disk Access、keyring unavailable、scheduler install mismatch。
- [x] `M2-TX-AU-005` 在 Dashboard / Settings / Audit 之間建立清晰跳轉，讓使用者能從問題卡片直接進到對應修復入口。

### I18n Architecture

- [x] `M2-TX-I18N-001` 拆解現有 [`src/lib/i18n.ts`](../../../src/lib/i18n.ts) 巨檔，建立 namespace-based 的翻譯結構。
- [x] `M2-TX-I18N-002` 凍結 day-one 語系範圍：`en`、`zh-CN`、`zh-TW`，明確哪些內容必須同步交付。
- [x] `M2-TX-I18N-003` 定義翻譯 key naming convention、插值規則、日期時間 / 數字 / 檔案大小格式化策略。
- [x] `M2-TX-I18N-004` 將 trust-critical 文案列為高優先翻譯項，不允許在核心操作中混入英語 fallback。
- [x] `M2-TX-I18N-005` 為 route title、warning、button label、empty state、audit artifact label 建立翻譯 coverage 檢查。
- [x] `M2-TX-I18N-006` 建立 pseudo-locale 或等價測試機制，提早發現字串溢出和 layout 破壞。

### Platform Expansion

- [x] `M2-TX-PL-001` 定義 Windows scheduler PME story，至少包含 preview、manual instructions、apply / remove plan。
- [x] `M2-TX-PL-002` 定義 Linux scheduler PME story，至少涵蓋 systemd user service 或等價方案和手動 fallback。
- [x] `M2-TX-PL-003` 實作平台 capability detection，讓 UI 可根據 macOS / Windows / Linux 顯示對應文案和限制。
- [x] `M2-TX-PL-004` 將 Safari / macOS Full Disk Access guidance 做成可重用 component 和 help entry。
- [x] `M2-TX-PL-005` 為 Linux keyring 不可用情境建立正式 UX：哪些功能可退化、哪些流程必須阻擋。
- [x] `M2-TX-PL-006` 在 Settings / Schedule / Security 頁加入平台-specific troubleshooting 和 docs 入口。

### Accessibility And Interaction Quality

- [ ] `M2-TX-AX-001` 為所有核心流程完成 keyboard-only walkthrough，修正 focus trap、tab order、dialog announcement。
- [ ] `M2-TX-AX-002` 為圖表、audit severity、status chips、timeline 等建立文字替代和 screen-reader friendly label。
- [ ] `M2-TX-AX-003` 在 reduced motion 模式下降低動畫，尤其是 panel transition、chart animation、loading shimmer。
- [ ] `M2-TX-AX-004` 為多語系字串長度變化重新校正 layout，避免 prototype 視覺在真實內容下崩壞。

### Testing And Acceptance

- [ ] `M2-TX-QA-001` 建立 PME 跨流程 acceptance tests，覆蓋 import、rollback、rekey、doctor repair。
- [x] `M2-TX-QA-002` 建立 i18n coverage test，確保核心語系不存在缺 key 和不當 fallback。
- [ ] `M2-TX-QA-003` 建立 Windows / Linux 平台 capability 和 schedule guidance 的 contract tests。
- [x] `M2-TX-QA-004` 進行至少一輪 accessibility review 和一輪多語系 visual QA。
- [x] `M2-TX-QA-005` 把所有 trust UX 的 user-facing 文案和 warning style 回寫到 docs 或 design notes，避免後續被無聲改壞。

---

## Exit Artifacts

- PME 互動語法正式化
- Trust / Audit UX 加強
- `en` / `zh-CN` / `zh-TW` 核心語系
- Windows / Linux 平台支持基礎
