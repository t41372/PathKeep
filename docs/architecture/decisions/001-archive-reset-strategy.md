# ADR-001: Archive Reset Strategy

## Status

Accepted

## Context

PathKeep 目前倚賴的 archive schema 來自 `browser-history-backup` 時代，是在既有 SQLite 結構上持續疊加欄位與表的結果。現行
`src-tauri/crates/vault-core/src/archive-schema.sql` 已經能看出幾個明顯問題：

- schema 是按功能逐步補丁演化出來的，缺少以 PathKeep 當前產品模型為中心的 canonical naming 和 table boundary。
- 時間欄位、run ledger、import batch、profile metadata 等概念還停留在舊產品假設上，無法直接承載新的多瀏覽器、rollback、audit 和 PME 流程。
- 舊 schema 透過 `ensure_column()` 之類的 ad-hoc bootstrap 維持可運行，代表每次調整都要先和歷史包袱協商，會拖慢 M0 / M1 的資料平面重建。

M0 接下來要落地 canonical schema v1、migration ledger、unified run model 和 rollback visibility。如果繼續在 legacy schema 上原地演化，後續每個決策都會被舊表名、舊欄位和舊操作模型綁住。

另一方面，專案目前沒有正式用戶，也沒有必須維持向下相容的已發佈版本；但現有 archive 資料仍需要可恢復、可檢查，不能因為重寫而失去升級路徑。

## Decision

PathKeep 採用 **fresh schema** 策略作為 canonical archive 的起點。

- canonical schema v1 直接依照新的 architecture / feature docs 設計，不以現有 legacy archive schema 為基底繼續演化。
- 不要求舊 `browser-history-backup` schema 與新 schema 在表名、欄位或 migration path 上保持雙向相容。
- 對既有 archive DB，提供**一次性升級轉換工具**：讀取 legacy DB，寫入新的 canonical schema v1，並保留原始 DB 作為 recovery source。
- 在 canonical schema v1 建立之後，未來的 PathKeep schema 變更一律走正式 migration system，而不是回到 ad-hoc `ensure_column()` 模式。

## Consequences

- M0 後續的 schema、run ledger、rollback visibility 和 timestamp contract 可以直接對齊新產品模型，不需要為舊命名和舊欄位妥協。
- 我們必須實作一條明確的 legacy-to-v1 升級路徑，至少包含：
  - 偵測舊 archive schema
  - 建立新 v1 DB
  - 執行資料搬運與欄位映射
  - 產出 upgrade report / audit artifact
  - 保留原始 DB 或 migration 前 snapshot，確保 recoverability
- `src-tauri/crates/vault-core/src/archive-schema.sql` 從現在開始只視為 legacy reference；新的 canonical schema 會以 migration 檔案與 ADR 為 source of truth。
- 測試與驗收需要分成兩條路徑：fresh init（直接建立 v1）和 one-time upgrade（從 legacy DB 轉入 v1）。
