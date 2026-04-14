# Browser Schema Evolution Guide

> 當 Chrome / Firefox / Safari / Takeout schema 變動，或某個 fork 開始提供新的 metadata 時，依這份流程更新 PathKeep。

## Procedure

1. **取樣**
   - 取得最小可重跑 fixture
   - 若是 live browser DB，先做 staging snapshot / checkpoint
   - 若是 export / Takeout，保留原始檔案副本
2. **建立 schema diff artifact**
   - dump table list、column list、`sqlite_master.sql`
   - 記錄 browser family / product / version / source kind
   - 產生新的 `schema_fingerprint`
3. **欄位分類**
   - canonical fact
   - typed evidence
   - native entity
   - raw-artifact-only（必須寫理由）
4. **更新 extractor registry**
   - required / optional tables
   - capability tags
   - coverage notes
   - warning / degrade policy
5. **補 fixture 與測試**
   - happy path
   - missing optional table / column
   - unknown new field
   - partial-coverage / parse-warning case
6. **評估 perf / size impact**
   - source-evidence 寫入放大量
   - bundle size 變化
   - diagnostics/accounting 是否要補欄位
7. **同步 docs**
   - `docs/architecture/data-model.md`
   - `docs/architecture/browser-support-and-adapter-playbook.md`
   - `docs/features/deterministic-intelligence.md`（若 capability contract 受影響）
   - relevant ADR / plan docs

## Required Output

- schema diff artifact
- capability snapshot before / after
- updated fixture set
- perf / size impact note
- docs change list
- honesty note：是否影響 deterministic evidence boundary

## Do Not

- 不要只看 browser version 就假設欄位存在
- 不要把新欄位直接塞回 hot canonical visit row
- 不要在沒有 fixture / diff artifact 的情況下改 extractor
