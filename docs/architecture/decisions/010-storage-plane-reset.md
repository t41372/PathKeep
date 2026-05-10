# ADR-010: Storage Plane Reset

## Status

Accepted

Amended for v0.1.0 release scope (2026-04-29): the storage-plane boundary
still treats semantic vectors and readable-content blobs as rebuildable sidecar
state, but v0.1.0 does not ship LanceDB/vector runtime or webpage body fetch.

Amended for v0.2.0 release scope (2026-05-10): the same boundary remains in
force. v0.2.0 still does not ship LanceDB/vector runtime or webpage body fetch;
those blockers moved to the v0.3.0 roadmap.

## Context

PathKeep 目前的實作已經脫離最早的 legacy archive schema，但 repo 內仍保留一個 transitional 狀態：

- canonical archive、FTS、AI queue、deterministic runtime、assistant trace、部分 semantic compatibility rows 同住一個 hot SQLite
- `archive/schema.rs` 仍以 runtime bootstrap / backfill / compatibility views / `ensure_*_column()` 維持不同時代的表面共存
- `raw_row_versions` 在 hot archive 長期保存每筆來源 row 的 JSON payload
- `ai_embeddings` 同時保存 vector payload 與 content text，而真正的 ANN retrieval 又已交給 LanceDB sidecar

這種結構對小型測試資料仍能工作，但和新的 baseline 不相容：

- 4 核 3GHz CPU / 8GB RAM
- 60 年中度使用、至少 1440 萬 visits
- Chrome 約 18 個月一次性導入、Firefox 約 10 年一次性導入
- 需要容忍更大的一次性導入與長期增量成長

在這個 baseline 下，真正會先爆掉的不是 canonical facts，而是 hot SQLite 內混住的 FTS、runtime jobs、readable-content、embedding mirror 與整批 derived tables。既然專案目前沒有正式用戶、沒有發佈版本、只有測試資料，為了拿到最理想的長期架構，最合理的做法是直接 hard reset，而不是再為 transitional DB 設計升級敘事。

## Decision

PathKeep 改採 **4-layer storage plane**，並以 hard reset 方式直接切換：

1. `archive/history-vault.sqlite`
   - 只保存 canonical archive 與 immutable audit facts
   - 包含 `runs`、`source_profiles`、`urls`、`visits`、`downloads`、`search_terms`、`favicons`、`profile_watermarks`、checkpoint / manifest / import facts
2. `derived/history-search.sqlite`
   - 只保存 lexical recall 與統計 projection
   - 包含 FTS5、Explorer keyword recall projection、Dashboard / deterministic baseline 需要的 rollups
3. `derived/history-intelligence.sqlite`
   - 只保存 rebuildable intelligence runtime / read model
   - 包含 enrichment queue、assistant trace、deterministic features / groups / threads / cards / snapshot payload
4. `sidecars/`
   - `semantic-index/` 保留給 future vector sidecar；v0.2.0 不寫入 LanceDB vectors / ANN index
   - `intelligence-blobs/` 保留給 future content-addressed 正文 blob；v0.2.0 不 fetch / 保存網頁正文

同時採用以下硬邊界：

- 不再為 pre-reset archive DB 設計 migration、upgrade path 或 compatibility docs。
- 不再保留 `profiles` / `visit_events` compatibility views、legacy triggers、或 runtime `ensure_*_column` ad-hoc patching。
- 不再把 per-row raw JSON payload 視為 hot archive contract；raw capture 改為 checkpoint-first，保存 source checkpoint、schema fingerprint、manifest 與 canonical facts。
- 不再把 SQLite `ai_embeddings` 當作向量 payload mirror；semantic metadata 與 future vector payload 分離。v0.2.0 不 shipping ANN retrieval。

## Consequences

- PathKeep 的 source of truth 會更簡單：canonical facts、lexical recall、intelligence runtime、semantic vectors 各自有清楚邊界，不再彼此拖慢。
- 既有 archive DB 在 reset 後不會被打開或升級；唯一 supported path 是刪除本地資料後重新從瀏覽器 / Takeout 導入。
- 文檔必須同步移除任何「legacy upgrade path」、「migration ledger」、「compatibility bridge」仍是正式策略的敘述。
- Rust 實作必須把 browser snapshot、streaming ingest、projection catch-up、deterministic rebuild resume 等能力收斂成 long-horizon baseline，而不是再依賴 hot SQLite 共住 derived state。
- 測試與 benchmark 必須改成以 fresh init / re-import 為主，不再維護 legacy DB compatibility acceptance。

## 2026-04-14 Closeout Note

- `archive/history-vault.sqlite`、`derived/history-search.sqlite`、`derived/history-intelligence.sqlite` 與 `sidecars/{semantic-index,intelligence-blobs}` 已全部落地成 repo 目前的正式 storage plane。
- canonical archive 已移除 `raw_row_versions` hot-path persistence；來源證據改由 checkpoint / snapshot / manifest trace 承接。
- intelligence plane 不再靠 temp compatibility views 或 runtime `ensure_*_column` patching 存活；讀路徑直接使用 attached `archive.*` 表。
- SQLite 只保留 compact semantic metadata / rebuild accounting；v0.2.0 不寫入 LanceDB vector sidecar，也不 fetch / 保存可讀正文。`sidecars/{semantic-index,intelligence-blobs}` 是後續版本重新開放時的可重建 derived-state 邊界。
