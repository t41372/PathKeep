# ADR-011 — Source-Evidence Archive And Capability-Driven Ingest

## 狀態

Accepted (2026-04-14)

## 背景

`ADR-010` 已經把 PathKeep 的 storage plane 從「一個 hot SQLite 混住所有東西」重置成 canonical/search/intelligence/sidecars 四層。這個方向是對的，但 repo 仍保留一個重大缺口：

- `browser-history-parser` 目前只輸出窄版 parsed rows，會在 ingest 前就失去大量 browser-specific metadata
- `source_profiles` 只保留有限 provenance；每次 backup/import 的 browser version、schema fingerprint、capability 與 coverage 沒有形成正式 batch history
- raw source checkpoint 雖然保留了來源證據，但不應該是唯一的 field-preservation story
- intelligence / feature enablement 仍容易被寫成 browser/version-driven，而不是 capability-driven

PathKeep 的目標不是只保存「今天剛好要查的欄位」，而是長期保留 browser-native evidence，讓未來新增 intelligence、browser schema 演進、以及跨 family / fork 的 graceful degradation 都有正式 contract。

## 決策

### 1. Archive plane 擴成 hot/cold split

PathKeep 的 archive plane 改定義為兩個子平面：

- `archive/history-vault.sqlite`
  - hot canonical facts
  - 只保存高頻 query / sort / filter / rollback / restore 一定要用到的 canonical rows
- `archive/source-evidence.sqlite`
  - cold archived source-native evidence
  - 保存 typed evidence、native entities、schema observation、source capability 與 source batch metadata

`archive/source-evidence.sqlite` 屬於 **archive contract**，不是 rebuildable derived state；remote/local backup bundle 必須把它與 `archive/history-vault.sqlite` 一起打包。

### 2. Provenance 以 profile + batch 兩層表達

`source_profiles` 只保存穩定身份：

- `browser_family`
- `browser_product`
- `profile_key`
- `profile_name`
- `profile_path`
- optional user/account hints

每次 backup / import 的來源觀測改落在 `source_batches`：

- `source_profile_id`
- `source_kind`
- `browser_version`
- `schema_version_text`
- `schema_version_int`
- `schema_fingerprint`
- `parser_version`
- `capability_snapshot_json`
- `coverage_stats_json`
- source artifact refs / notes

`profile_watermarks` 改為回鏈最後成功 ingest 的 `source_batch_id` 與該 batch 對應的 cursors / fingerprints。

### 3. Extractor contract 改為 capability-driven、不是 row-only parser

`browser-history-parser` 的正式輸出不再只是一組窄版 parsed rows，而是 family extractor contract：

- `SchemaObservation`
- `CapabilitySnapshot`
- `CanonicalFactsBatch`
- `TypedEvidenceBatch`
- `NativeEntityBatch`
- `ParserWarning`

Extractors 一律遵守：

- **introspection first, version second**
- 先看 table / column / parse success / actual coverage
- browser version、schema version 只作 heuristics / debug / diff 輔助

### 4. source-evidence plane 採 hybrid 結構

`archive/source-evidence.sqlite` 內至少有：

- typed evidence tables
  - `visit_search_evidence`
  - `visit_navigation_evidence`
  - `visit_engagement_evidence`
  - `visit_context_evidence`
- cold native archive
  - `native_entities`

`native_entities` 用來保存：

- 非 visit 粒度的 source-native data
- 暫時還沒有 promotion 成 typed evidence 的欄位 / entity
- 例如 Firefox `moz_inputhistory` / metadata、Chromium clusters / annotations / task graph、Safari tombstones / tags、Takeout Session / tab navigation

payload 預設採壓縮後的 JSON/CBOR；超大 payload 以 content-addressed blob ref 指向受管 sidecar。

### 5. Unknown field policy 改成 preserve by default

遇到 extractor 尚未識別的新欄位時：

1. 不得直接丟棄
2. 優先進 `native_entities`
3. 只有明確判定為低價值、且可以從 retained raw artifact 無損重建的資料，才可不 promotion 到 archived typed/native evidence

### 6. Intelligence contract 改成 capability-driven

所有 deterministic / optional AI module 必須宣告：

- required capabilities
- optional enhancement capabilities
- fallback / degrade behavior

例如：

- `search.native_terms`
- `nav.from_visit`
- `nav.opener_visit`
- `engagement.foreground_ms`
- `context.task_graph`

browser/version metadata 保留，但只用於 provenance、compatibility reporting、extractor heuristics 與 debugging；**不可**作為 intelligence feature enablement 的主 contract。

### 7. Raw artifacts 仍保留，但角色降級為 debug / diff / re-extract evidence

raw source artifacts 仍然保留：

- import source（Takeout / export）copy 進 managed artifact store
- live browser backup 保留 latest successful checkpoint per profile
- schema-fingerprint-change checkpoint
- restore / rekey safety snapshot

但 raw artifact 不再承擔唯一的 field-preservation 責任；長期產品 contract 以 archived typed/native evidence 為主。

## 理由

- **長期韌性**：只靠 raw checkpoint，代表每次新 intelligence 都要重新打開 source artifact；成本高，contract 模糊。
- **熱路徑性能**：把 cold native evidence 移出 hot canonical，可保留欄位，同時不把主查詢拖慢。
- **跨瀏覽器誠實性**：capability snapshot 比 browser/version branch 更能支撐 Chromium forks、舊版 schema、partial coverage 與 graceful degradation。
- **可維護性**：extractor registry + typed/native evidence 層，能避免 downstream intelligence 邏輯一直直接耦合到 family-specific table/column。
- **未來 promotion 成本低**：native field 先保存，再以 `field promotion` / `re-extract` job 升格成 typed evidence 或 canonical-derived contract，不需要重新叫使用者導入。

## 後果

### 正面

- PathKeep 會有更完整的 source provenance 與 capability history
- browser-specific metadata 可以被長期保留，而不污染 hot canonical visit rows
- new browser schema / new intelligence feature 的升級路徑會更清楚
- Takeout 可以和 Chromium / Firefox / Safari 一起收斂到同一個 extractor contract

### 負面

- archive bundle 體積會增加，因為 `archive/source-evidence.sqlite` 屬於正式 archive contract
- extractor / ingest / diagnostics 的實作會變複雜，需要 registry、coverage 統計與更多 fixture
- source-evidence DB 與 raw artifact retention policy 需要更完整的 storage accounting 與 prune UX

## 相關

- [010-storage-plane-reset.md](010-storage-plane-reset.md)
- [../data-model.md](../data-model.md)
- [../module-boundary-map.md](../module-boundary-map.md)
- [../browser-support-and-adapter-playbook.md](../browser-support-and-adapter-playbook.md)
- [../../features/deterministic-intelligence.md](../../features/deterministic-intelligence.md)
- [../../plan/program/research-and-decisions.md](../../plan/program/research-and-decisions.md)
