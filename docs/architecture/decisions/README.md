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

以下決策都已在 planning docs 中有明確結論，
agent 只需要把它們正式化成 ADR 文件即可：

1. **ADR-001** — [Fresh schema](001-archive-reset-strategy.md)（不在 legacy schema 上繼續演化；既有 DB 走一次性 upgrade）
2. **ADR-002** — [Canonical timestamp contract](002-timestamp-contract.md)（`*_ms` + `*_iso` + run timezone + fallback timezone）
3. **ADR-003** — [Unified run ledger](003-run-model.md)（所有操作類型共用一張 `runs` 表）
4. **ADR-004** — [Soft-hide rollback](004-rollback-visibility-model.md)（用 `reverted_at` / `reverted_by_run_id` 標記可見性，不刪 immutable facts）
5. **ADR-005** — [App Lock session boundary](005-app-lock-session-boundary.md)（App Lock 保護 UI session 與 read/query surface；archive encryption 仍獨立保護資料庫檔案）
6. **ADR-006** — [Deterministic intelligence boundary](006-deterministic-intelligence-boundary.md)（Accepted：用 honest evidence / query groups / rule-first taxonomy 取代 session / dwell-centric deterministic baseline）
7. **ADR-007** — [macOS Touch ID session unlock](007-macos-biometric-session-unlock.md)（Accepted：macOS Touch ID 是 additive session convenience，不改寫 ADR-005 的 session-only boundary）
8. **ADR-008** — [Consented frontend analytics boundary](008-consented-frontend-analytics-boundary.md)（Accepted：analytics 必須 explicit opt-in、frontend-only、coarse、first-party JSON、無 hidden telemetry）
9. **ADR-009** — [Default desktop optional intelligence shipping](009-default-desktop-optional-intelligence-shipping.md)（Accepted：optional AI / MCP / semantic runtime 維持與 default desktop binary 一起 in-process shipping）
