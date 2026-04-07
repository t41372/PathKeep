# Architecture Decision Records (ADR)

> 這裡放所有正式的技術決策記錄。每個 ADR 對應 `docs/plan/STATUS.md` 中的一個 decision bundle 或 work block。

## 命名規則

```
001-archive-reset-strategy.md
002-timestamp-contract.md
003-run-model.md
004-rollback-visibility-model.md
```

## ADR 模板

```markdown
# ADR-XXX — [決策標題]

## 狀態

Accepted | Proposed | Superseded

## 背景

[為什麼需要做這個決策]

## 決策

[我們決定了什麼]

## 理由

[為什麼這樣做比較好]

## 後果

[這個決策帶來的影響，包括正面和負面]

## 相關

- STATUS.md 對應的 work block / decision bundle
- [相關文檔連結]
```

## 已知決策（已有結論，需要正式寫成 ADR）

以下四個決策已經在 planning docs 中有明確結論，
agent 只需要把它們正式化成 ADR 文件即可：

1. **ADR-001** — [Fresh schema](001-archive-reset-strategy.md)（不在 legacy schema 上繼續演化；既有 DB 走一次性 upgrade）
2. **ADR-002** — [Canonical timestamp contract](002-timestamp-contract.md)（`*_ms` + `*_iso` + run timezone + fallback timezone）
3. **ADR-003** — [Unified run ledger](003-run-model.md)（所有操作類型共用一張 `runs` 表）
4. **ADR-004** — [Soft-hide rollback](004-rollback-visibility-model.md)（用 `reverted_at` / `reverted_by_run_id` 標記可見性，不刪 immutable facts）
