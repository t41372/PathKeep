# ADR-004: Rollback Visibility Model

## Status

Accepted

## Context

PathKeep 的核心承諾之一是 recoverability。Import、future rollback、doctor repair 或 snapshot restore 不能透過 destructive delete 來「假裝回到之前」。同時，並不是所有表都該支援同一種 rollback 行為：

- raw capture、manifest、snapshot metadata 本身是 immutable audit facts。
- user-visible canonical facts（visits、downloads、search_terms）需要在 rollback 後被隱藏，但仍保留可審計歷史。
- FTS、aggregation、AI / insight sidecars 都屬於 derived state，應該可以重建，而不是把 rollback 細節寫進每一張衍生表。

## Decision

PathKeep 採用 **soft-hide rollback** 模型：

- `visits`、`downloads`、`search_terms` 等 user-visible fact tables 以 `reverted_at` / `reverted_by_run_id` 表示邏輯上已回滾。
- `manifests`、`snapshots`、`schema_migrations` 與 source checkpoint trace 視為 immutable audit facts，不做 soft-delete。
- `urls` / `source_profiles` 作為 canonical anchors，不直接刪除；它們的可見性由關聯 fact rows 的存在與 read model 判定。
- FTS projection、aggregation tables、AI / insight sidecars 一律視為 derived state。rollback 之後以 rebuild / invalidate 解決，不在每張 derived table 內複製一套 `reverted_at` 欄位。

## Rationale

- 這個模型同時滿足 recoverability 和 auditability：我們能隱藏錯誤導入的資料，又不會丟失它曾經存在的事實。
- raw capture 保持 immutable，讓 parser bug、upgrade bug 或 user 誤操作之後仍有能力做 doctor / replay / re-import。
- 把 rollback visibility 收斂到 canonical facts，可避免 derived state 爆炸式複製邏輯。

## Consequences

- 所有 user-facing query 都必須預設只讀取 `reverted_at IS NULL` 的 canonical facts。
- rollback UI 需要以 run 為中心呈現「哪些資料會被隱藏、哪些 artifact 會保留、哪些 derived states 會重建」。
- FTS 與 aggregation 不能被視為 source of truth；它們在 rollback 後的正確做法是重建。

## Related

- `WORK-M0-A`
- [docs/architecture/data-model.md](../data-model.md)
- [docs/features/archive.md](../../features/archive.md)
- [docs/plan/program/research-and-decisions.md](../../plan/program/research-and-decisions.md)
