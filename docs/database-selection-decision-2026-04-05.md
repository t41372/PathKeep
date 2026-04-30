# PathKeep 數據庫選型決策

> Status: Accepted
> Date: 2026-04-05
> Scope: Next-generation architecture
> Canonical requirements source: `docs/vision-and-requirements.md`

> v0.1.0 release amendment (2026-04-29): Vector / semantic retrieval remains a
> future replaceable sidecar concept, but the default v0.1.0 build does not
> include LanceDB / vector runtime, and the UI must present semantic search,
> embedding, assistant, MCP, and readable webpage body fetch as deferred.

## 1. Executive Summary

採用 `SQLite-first layered architecture`，不是一個 "all-in-one" database。

最終選型：

- Canonical archive database: **SQLite**
- Lexical recall projection database: **SQLite**
- Intelligence runtime projection database: **SQLite**
- Encryption: **SQLCipher**
- Full-text recall: **SQLite FTS5**
- Vector / semantic retrieval sidecar: **future replaceable sidecar** (LanceDB
  was the earlier accepted candidate; v0.1.0 does not ship it)
- AI / Embedding framework: **rig.rs**
- Heavy analytics sidecar: **不在 day one 引入**；只有在後續 benchmark 證明必要時才加入 DuckDB

這是最適合本產品實際形狀的方案：

- local-first
- single-user desktop application
- 20+ year data longevity
- append-only archive semantics
- auditability and rollback
- AI as optional, rebuildable derived state

最重要的架構規則：

> The canonical source of truth must remain in SQLite.
> Embeddings, vector indexes, topic clusters, summaries, and other AI assets are derived state and must remain rebuildable.

---

## 2. Hard Constraints From Product Requirements

Based on `docs/vision-and-requirements.md`, the database design must satisfy all of the following:

- Data stays local and user-accessible.
- Archive remains readable and maintainable over 20+ years.
- Core backup and recall work without any AI configuration.
- The archive supports append-only facts, row versioning, audit manifests, and reversible rollback.
- The system works as a desktop app without requiring a permanent background server.
- Large imports and long-term heavy usage remain operational on consumer hardware.
- Full-text recall must be first-class.
- Semantic search is important, but it is not source of truth and can be rebuilt.

---

## 3. Final Architecture Decision

### 3.1 Canonical Archive: SQLite

SQLite is the correct canonical database for the archive plane.

Why:

- It is the best match for a local-first desktop application.
- It has the strongest long-term file-format credibility of any candidate.
- It keeps the user's data as a directly owned file rather than a managed service.
- Its single-writer model fits this product's write pattern: background ingest plus interactive reads.
- It supports the required schema richness for raw capture, canonical facts, runs, rollback state, aggregates, and operational metadata.

What SQLite owns:

- canonical normalized facts
- run ledger and audit state
- rollback / tombstone state
- lexical recall projections and aggregates
- intelligence runtime projections and read models

What SQLite must not be forced to own:

- long-term ANN serving as its primary responsibility
- heavyweight OLAP marts unless proven necessary
- experimental vector indexing features as a hard dependency

### 3.2 Encryption: SQLCipher

SQLCipher is the correct encryption layer.

- It preserves the SQLite mental model and API shape.
- It uses transparent page-level encryption.
- It supports a clean encrypted / unencrypted / rekey lifecycle.
- Encrypted mode uses SQLCipher; unencrypted mode uses plain SQLite.
- The archive schema and application logic must remain SQLite-native so both modes behave the same above the storage layer.

### 3.3 Full-Text Recall: SQLite FTS5

FTS5 is the correct search layer for lexical recall.

- Recall is a core feature, not an optional plugin.
- FTS5 is mature, embedded, and already aligned with the canonical archive.
- Use FTS5 as an index/projection layer over canonical documents.
- Only index text needed for recall (title, URL, search terms, labels, selected enrichment text).
- Do not duplicate all enrichment text blindly into FTS.

### 3.4 Vector / Semantic Search: Future Replaceable Sidecar

LanceDB was the accepted candidate sidecar, driven by rig.rs for embedding
generation. The 2026-04-29 v0.1.0 release amendment removes the direct LanceDB
runtime from the default build until AI / semantic search has new runtime truth,
packaging, release-size, and supply-chain evidence.

Why LanceDB was accepted as the earlier candidate:

- It is local and embedded, which fits the desktop product shape.
- It supports vector indexing (IVF-PQ), quantization, and hybrid retrieval.
- It has a realistic path to handling large local datasets with disk-based indexing.
- It can remain a derived-state sidecar instead of forcing vector infrastructure into the canonical archive.

Why rig.rs:

- Rust-native LLM/Embedding framework, naturally integrates with our Rust workspace.
- Unified provider abstraction for Ollama, OpenAI-compatible, Anthropic, Google APIs.
- Handles both embedding generation and LLM inference for Intelligence features.

Operational rule:

- The vector layer is intentionally replaceable.
- The system must be able to delete and rebuild the entire vector sidecar from canonical SQLite data.

### 3.5 Heavy Analytics: DuckDB Deferred

DuckDB is not selected as a day-one required database, but it remains the leading analytics sidecar candidate if later needed.

When DuckDB becomes justified:

- long-window comparative analytics become expensive in SQLite
- the team needs repeated heavy ad-hoc analysis over large derived datasets
- insight generation benefits from separate OLAP storage and execution

If added later, DuckDB must remain: optional, derived, rebuildable, non-authoritative.

---

## 4. Architecture Rules

### 4.1 Source Of Truth Boundary

Only `archive/history-vault.sqlite` may hold canonical source-of-truth archive data.

This includes:

- visits
- URL metadata versions
- import and backup runs
- rollback state
- checkpoint / manifest facts

This explicitly excludes:

- embeddings
- ANN indexes
- topic clusters
- thread assignments
- generated summaries
- rerank caches

`derived/history-search.sqlite` 與 `derived/history-intelligence.sqlite` 都可以使用 SQLite，但它們不是 canonical archive。它們只保存可重建 projection / runtime state。

### 4.2 Rebuildability

All AI-derived state must be deletable and rebuildable from canonical archive facts.

The product must be able to:

- drop the LanceDB vector sidecar
- rebuild embeddings (via rig.rs)
- rebuild vector indexes
- rebuild topic and thread derived state

without losing archive truth.

### 4.3 Adapter Boundary

The vector layer must be hidden behind a storage adapter.

The application must not hard-code LanceDB-specific assumptions into:

- canonical schema
- audit model
- rollback model
- import semantics

This preserves the option to move later to Qdrant Edge, a future SQLite-native ANN extension, or another embedded vector engine.

### 4.4 Milestone Discipline

- M1 and M2 should ship without any required vector sidecar.
- M3 introduces the LanceDB sidecar for semantic search via rig.rs.
- DuckDB should only appear after concrete benchmark pain justifies it.

---

## 5. Revisit Triggers

This decision should be revisited only if one of the following becomes true:

- LanceDB becomes a poor operational fit under real-world local datasets.
- Qdrant Edge reaches clear production maturity and demonstrates a better embedded Rust integration story.
- SQLite-native ANN options (e.g. Vec1) become production-grade and materially simplify the stack.
- Recall and insights workloads prove that SQLite alone is insufficient for derived analytics.
- Product direction changes from local-first desktop archive to a multi-user or service-hosted system.

---

## 6. Final Decision Statement

The next-generation PathKeep standardizes on:

- **SQLite** as the canonical archive database
- **SQLCipher** as the encryption layer
- **SQLite FTS5** as the lexical recall layer
- **LanceDB** as the replaceable vector sidecar for semantic retrieval
- **rig.rs** as the Rust-native AI/Embedding framework
- **DuckDB** only as a future optional analytics sidecar, never as source of truth

This decision maximizes long-term durability, local ownership, auditability, and architectural flexibility while keeping AI capabilities powerful but non-authoritative.

---

## 7. Key References

- [SQLite Long Term Support](https://sqlite.org/lts.html)
- [SQLite Limits](https://sqlite.org/limits.html)
- [SQLite FTS5](https://sqlite.org/fts5.html)
- [SQLite Backup API](https://sqlite.org/backup.html)
- [SQLCipher Design](https://www.zetetic.net/sqlcipher/design/)
- [LanceDB Documentation](https://docs.lancedb.com/)
- [LanceDB Vector Indexes](https://docs.lancedb.com/indexing/vector-index)
- [rig.rs](https://github.com/0xPlaygrounds/rig)
- [DuckDB Concurrency](https://duckdb.org/docs/current/connect/concurrency)
