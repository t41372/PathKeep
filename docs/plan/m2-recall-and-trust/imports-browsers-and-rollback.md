# M2-IR — Imports, Browsers, And Rollback

> 讀這份文檔的時機：當你要把 PathKeep 從「會備份 Chromium」提升到「能吸收多來源歷史、能安全回滾、能自我診斷」的可信產品。  
> 這份文檔是 Recall & Trust 的資料層和操作層主計劃。
>
> **狀態註記（2026-04-07）**：`WORK-M2-A` 已完成。現有基線包含 Google Takeout dry-run / preview / quarantine / import / revert / restore、Firefox 正式 ingest、Safari history-only baseline ingest + Full Disk Access guidance，以及 doctor / repair 對 import artifact、visibility、derived state 的修復。剩餘 PME UI / trust copy / reusable guidance component 收斂到 `M2-TX`。
>
> **2026-04-13 truth-pass 註記；2026-04-24 Atlas / Comet promotion follow-up**：這裡記錄的是 M2 已經落地的 implementation coverage，而不是今天的 public promise。Firefox 與其他 adapter 仍保留實作；README / onboarding / release docs 現在只公開承諾 Google Chrome、macOS ChatGPT Atlas browser-history profile、macOS Perplexity Comet browser-history profile、與授權 Full Disk Access 後的 macOS Safari baseline，直到有獨立 promotion evidence 才能再提升其他 adapter。

---

## Source Inputs

- [../../features/archive.md](../../features/archive.md)
- [../../features/recall.md](../../features/recall.md)
- [../../architecture/data-model.md](../../architecture/data-model.md)
- [../program/research-and-decisions.md](../program/research-and-decisions.md)
- [../m1-solid-archive/schema-backup-and-ledger.md](../m1-solid-archive/schema-backup-and-ledger.md)
- [../m1-solid-archive/schedule-security-and-storage.md](../m1-solid-archive/schedule-security-and-storage.md)

---

## 本工作包要交付什麼

- Google Takeout 的 dry-run、preview、quarantine、import、rollback
- Firefox ingest implementation 與 Safari 基礎支持
- rollback / un-revert / visibility model 的實際落地
- doctor 深化和資料完整性修復工具

---

## WBS

### Google Takeout Import

- [x] `M2-IR-TO-001` 定義 Google Takeout import artifact 流程：上傳、掃描、preview、quarantine、execute、audit artifact。
- [x] `M2-IR-TO-002` 實作 Takeout file validation，檢查格式、版本、必要欄位、大小、重複匯入風險。
- [x] `M2-IR-TO-003` 實作 dry-run parser 和 summary 生成，回報 URL / visit / search / download 等類別統計和異常。
- [x] `M2-IR-TO-004` 實作 quarantine storage，確保匯入前的中間檔和 preview artifact 可追蹤且可清理。
- [x] `M2-IR-TO-005` 實作 execute import pipeline，將 Takeout 資料寫入 canonical schema 並帶上 source / run metadata。
- [x] `M2-IR-TO-006` 為 Takeout import 建立 dedupe 和 identity policy，避免和既有 Chromium archive 重複灌入。
- [x] `M2-IR-TO-007` 為 Takeout import 生成完整 audit artifact：來源檔、摘要、row counts、warnings、quarantine path、rollback hint。
- [x] `M2-IR-TO-008` 為 Takeout import 建立失敗清理策略，避免半套匯入留下 visibility 和 artifact 汙染。

### Firefox And Safari Support

- [x] `M2-IR-BR-001` 將 Firefox backup pipeline 從 parser 到 canonical ingest 打通，包含 profile discovery、staging、watermark、run stats。
- [x] `M2-IR-BR-002` 為 Firefox 對齊 Explorer 和 Export query surface，確保 UI 能識別 source browser 和 profile。
- [x] `M2-IR-BR-003` 實作 Safari profile / path detection 的 macOS guidance，包含 Full Disk Access 缺失時的提示。
- [x] `M2-IR-BR-004` 完成 Safari 基礎 ingest，至少可導入 history records 並帶出清楚的限制和 unsupported warning。
- [x] `M2-IR-BR-005` 建立多 browser source normalization 規則，確保同一 URL / visit 概念在 Explorer 中可一致呈現。
- [x] `M2-IR-BR-006` 對每個 browser 明確標記 capability 和 caveat，避免 UI 暗示 Safari / Firefox 和 Chromium 完全同等。

### Rollback And Visibility

- [x] `M2-IR-RB-001` 將 M0 定義的 rollback visibility model 正式落地到 canonical tables 和 run ledger。
- [x] `M2-IR-RB-002` 實作 rollback preview，顯示將隱藏、恢復、保留的資料類型和預期影響範圍。
- [x] `M2-IR-RB-003` 實作 rollback execute，確保基於 run 或 artifact 進行的回滾可追蹤、可審核。
- [x] `M2-IR-RB-004` 實作 un-revert / restore visibility 流程，避免 rollback 只有單向不可逆。
- [x] `M2-IR-RB-005` 為 Explorer / Export / Dashboard / Insights query layer 實作 visibility-aware filtering，確保已回滾資料不會漏出。
- [x] `M2-IR-RB-006` 為 rollback 失敗、部分成功、需要手動修復的情境建立 audit 和 doctor bridge。

### Doctor And Repair

- [x] `M2-IR-DR-001` 擴展 doctor 檢查項，至少加入 orphaned rows、broken visibility references、stale derived index、missing Takeout artifacts。
- [x] `M2-IR-DR-002` 為每種 doctor finding 定義 repairability：auto-fix、guided manual fix、read-only warning。
- [x] `M2-IR-DR-003` 實作 doctor re-run 和 repair run，讓修復本身也成為 run ledger 中可見操作。
- [x] `M2-IR-DR-004` 為 damaged import / damaged rollback fixture 建立測試，確保 doctor 真能報出問題。

### Testing And Acceptance

- [x] `M2-IR-QA-001` 建立 Google Takeout end-to-end acceptance：dry-run、execute、rollback、un-revert。
- [x] `M2-IR-QA-002` 建立 Firefox end-to-end acceptance：profile discovery、backup、Explorer recall、export。
- [x] `M2-IR-QA-003` 建立 Safari baseline acceptance：permission denied、permission granted、import success、unsupported warning。
- [x] `M2-IR-QA-004` 建立 rollback visibility acceptance，驗證 UI、query、export 都不會漏出已回滾資料。
- [x] `M2-IR-QA-005` 建立 doctor acceptance，至少覆蓋 checksum drift、missing artifact、broken visibility、stale index 四類 finding。

---

## Exit Artifacts

- Google Takeout import pipeline
- Firefox ingest implementation
- Safari 基礎支持
- rollback / un-revert / doctor 強化
