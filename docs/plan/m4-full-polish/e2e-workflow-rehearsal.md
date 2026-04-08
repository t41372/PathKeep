# M4 — End-To-End Workflow Rehearsal

> 對應 `M4-RL-QA-002`。這份文檔定義 release polish 階段要反覆演練的真實使用者工作流，讓 browser-preview e2e、桌面真機 runbook、以及最後的設計 / UX traceability 用同一份 coverage inventory。

---

## Scope

- 這一輪 rehearsal **不包含** AI provider、semantic index rebuild、insight generation 的深度驗收；那些另由 M3 / M4-A intelligence acceptance 覆蓋。
- 這份 inventory 聚焦非 AI / insights 的主線：onboarding、backup、Explorer、Audit、Import、Schedule、Security、Settings remote backup。
- browser-preview e2e 主要驗證資訊架構、PME grammar、路由與 honest fallback；真正的 scheduler、keyring、installer、signing 仍屬 platform runbook。

---

## Workflow Inventory

### 1. First Run To First Backup To Audit Confidence

- 使用者從空 archive 進入 Dashboard，透過 onboarding 完成 storage / security 決策、初始化 archive、跑第一次 backup。
- 驗收重點：
  - Dashboard zero-state、onboarding、first backup、recent runs、Explorer、Audit Ledger 的導航能完整串起。
  - shell chrome 視覺語言不低於 prototype：sidebar sections、topbar search、primary CTA、dashboard stats / panels 要保持一致。
  - 所有 fallback 文案都要誠實，不可出現 raw i18n key 或假資料冒充真實狀態。

### 2. Shared Profile Scope And Regex Recall

- 使用者先在 shell chrome 切換共享 profile scope，再進 Explorer 做 keyword / regex recall，最後把目前可見結果匯出。
- 驗收重點：
  - shared profile scope 在 Topbar、Sidebar、Explorer 之間保持一致。
  - Explorer 若沿用 shared scope，必須明講 inheritance；若使用者改成 page-specific profile，page filter 優先。
  - regex mode 需要有顯式 affordance、valid / invalid feedback，以及在 invalid pattern 時阻止 export / query，不能讓使用者以為 app 卡死。

### 3. Takeout Import, Revert, Restore, And Doctor

- 使用者匯入 Takeout，先看 preview，再 execute，接著 review recent batch、revert / restore，最後跑 doctor。
- 驗收重點：
  - Import 必須維持 PME grammar，而不是直接把資料灌進 archive。
  - recent batch detail 要能顯示 visible / duplicate / imported rows 與 preview rows。
  - revert / restore 後 UI 狀態要切換清楚，doctor 要能把 trust / repair surface 接回來。

### 4. Schedule And Security Review Surfaces

- 使用者在已初始化 archive 上 review native schedule preview / manual steps，再回到 Security 檢查加密狀態與 recovery affordance。
- 驗收重點：
  - Schedule / Security 是 review surface，不是黑盒 execute-only 面板。
  - browser preview 要清楚標示哪些是 read-only 模擬、哪些操作必須在桌面 shell 或 OS 工具完成。
  - keyboard reachability、callout hierarchy、copyable manual steps 都要維持可用。

### 5. Remote Backup PME

- 使用者在 Settings 設定 remote backup、儲存 credentials、preview bundle、execute upload、verify bundle，最後回看 derived-state controls。
- 驗收重點：
  - `Preview / Manual / Execute / Verify` 四個 tab 都要有清楚的 boundary 文案。
  - preview / execute / verify 的 artifact 必須可見：bundle path、object key、upload URL、bundle version、restore readiness。
  - remote backup 的 honest fallback 要清楚區分「配置未完成」、「preview 還沒跑」、「verify 還沒跑」。

---

## Current Browser-Preview Coverage

- [`tests/e2e/shell.spec.ts`](../../../tests/e2e/shell.spec.ts) 已覆蓋 workflow 1、2、3、4、5 的 browser-preview 路由與核心交互。
- `src/app/index.test.tsx`、`src/pages/trust-flows.test.tsx`、`src/pages/intelligence-surfaces.test.tsx` 補 unit / product-flow 層的 state coverage，避免只剩 e2e smoke。

---

## Residual Gaps

- 真機 scheduler install / verify、keyring、biometric、installer、signing / notarization 仍需平台 runbook，不可被 browser preview e2e 取代。
- regex search 目前是 manual post-filter mode，不代表大數據量下已具備 release-grade performance baseline。
- Dashboard 的 shared profile scope 目前只保證 scoped insights / deep-link 誠實呈現；archive-wide aggregate KPIs 尚未做完整 per-profile partition。
