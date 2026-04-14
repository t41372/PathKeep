# ADR-002: Canonical Timestamp Contract

## Status

Accepted

## Context

PathKeep 要把 Chromium、Firefox、Safari、Google Takeout 等不同來源的時間格式收斂到同一套 canonical data plane。M0 要建立 schema v1、migration ledger 和 parser crate；如果 timestamp contract 仍停在「之後再決定」，那 `_ms` 欄位命名、rollback trace、run timezone metadata、前端顯示規則都會在後面反覆返工。

目前 planning docs 已經明確指出幾個方向：

- canonical 時間欄位應以 Unix epoch 毫秒整數為主，以支撐排序、範圍查詢、bucket aggregation 和 session/burst 計算。
- 仍需要一份 ISO 8601 UTC 輔助欄位，保留 debug、導出和人工 SQL 可讀性。
- run 本身要記錄執行當下的系統 timezone，讓本地時間推導可追溯。
- fallback timezone 需要作為設定值存在，供 Takeout 或舊資料缺乏 timezone metadata 時使用。

## Decision

PathKeep 凍結 canonical timestamp contract 如下：

- 所有 canonical 事件時間主欄位一律使用 `*_ms` 命名，型別為 `INTEGER NOT NULL`，語意為 **Unix epoch milliseconds in UTC**。
- 所有需要提供人類可讀與導出穩定性的對應欄位，一律使用 `*_iso` 命名，型別為 `TEXT`，內容為 **UTC ISO 8601**。
- canonical schema 中不保存「local time」欄位。local time 一律由 `*_ms` 加上 run 或 fallback timezone 在讀取端推導。
- `runs.timezone` 記錄當次 backup/import/doctor/revert/snapshot restore 執行時的系統 timezone（IANA TZ database name，如 `America/Phoenix`）。
- `settings` 中保留 `archive.fallback_timezone` 作為缺失 timezone metadata 時的回退值；未顯式設定時，預設使用當前系統 timezone。
- raw capture 層保留來源原始時間值，不對來源 snapshot / checkpoint 做 destructive rewrite。

## Rationale

- `INTEGER` 毫秒時間是 canonical archive 最適合的運算形式，符合大範圍時間軸、session 分段和統計窗口的長期需求。
- ISO 輔助欄位把 debug / export / audit 的可讀性保留下來，同時避免查詢時每次都重新格式化。
- timezone metadata 綁在 `runs` 上，符合「桌面 app 能知道當時系統 timezone」的產品能力，也避免把 timezone 分散塞進每一列 event。
- fallback timezone 以 settings 控管，可以明確承認歷史資料的不確定性，而不是假裝能精準還原所有過去的當地時間。

## Consequences

- canonical schema、parser crate 和所有 read models 都必須把 `_ms` / `_iso` 命名視為硬契約。
- 前端在沒有特殊說明時，顯示層一律以當前系統 timezone 呈現時間；當需要 audit 或 debug 時，可回到 `*_iso` 和 `runs.timezone`。
- FTS、aggregation 和 derived-state rebuild 必須以 `*_ms` 作為排序與 bucket 基準，不再依賴舊的混合 timestamp 命名。

## Related

- `WORK-M0-A`
- [docs/architecture/data-model.md](../data-model.md)
- [docs/plan/program/research-and-decisions.md](../../plan/program/research-and-decisions.md)
