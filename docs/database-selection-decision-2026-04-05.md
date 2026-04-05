# Browser History Vault Database Selection Decision

> Status: Accepted
> Date: 2026-04-05
> Scope: Next-generation architecture only
> Canonical requirements source: `docs/vision-and-requirements.md`

## 1. Executive Summary

The next-generation product should adopt a `SQLite-first layered architecture`, not a single "all-in-one" database.

Final selection:

- Canonical archive database: `SQLite`
- Encryption: `SQLCipher`
- Full-text recall: `SQLite FTS5`
- Vector / semantic retrieval sidecar: `LanceDB`
- Heavy analytics sidecar: `Do not adopt on day one`; add `DuckDB` only if later benchmarks prove it is necessary

This is the best fit for the product's actual shape:

- local-first
- single-user desktop application
- 20+ year data longevity
- append-only archive semantics
- auditability and rollback
- AI as optional, rebuildable derived state

The most important architectural rule is:

> The canonical source of truth must remain in SQLite.  
> Embeddings, vector indexes, topic clusters, summaries, and other AI assets are derived state and must remain rebuildable.

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

These constraints strongly favor embedded file-based storage for canonical data, and strongly disfavor making the core product depend on a server-style database or an immature vector engine.

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

- raw immutable capture
- canonical normalized facts
- run ledger and audit state
- rollback / tombstone state
- schema migrations
- recall projections and aggregates

What SQLite must not be forced to own:

- long-term ANN serving as its primary responsibility
- heavyweight OLAP marts unless proven necessary
- experimental vector indexing features as a hard dependency

### 3.2 Encryption: SQLCipher

SQLCipher is the correct encryption layer.

Why:

- It preserves the SQLite mental model and API shape.
- It uses transparent page-level encryption.
- It supports a clean encrypted / unencrypted / rekey lifecycle.
- It aligns with the product requirement that users may choose encryption without changing the rest of the architecture.

Decision:

- Encrypted mode uses SQLCipher.
- Unencrypted mode uses plain SQLite.
- The archive schema and application logic must remain SQLite-native so both modes behave the same above the storage layer.

### 3.3 Full-Text Recall: SQLite FTS5

FTS5 is the correct search layer for lexical recall.

Why:

- Recall is a core feature, not an optional plugin.
- FTS5 is mature, embedded, and already aligned with the canonical archive.
- It avoids introducing another service or operational dependency.
- It supports the product's search shape: URL, title, search terms, labels, selected enrichment text, and filtered timeline browsing.

Implementation rule:

- Use FTS5 as an index/projection layer over canonical documents.
- Only index text needed for recall.
- Do not duplicate all enrichment text blindly into FTS.

### 3.4 Vector / Semantic Search: LanceDB Sidecar

LanceDB is the chosen vector sidecar for the next-generation design.

Why it wins today:

- It is local and embedded, which fits the desktop product shape better than server-first vector systems.
- It supports vector indexing, quantization, and hybrid retrieval.
- It has a realistic path to handling large local datasets.
- It can remain a derived-state sidecar instead of forcing vector infrastructure into the canonical archive.

Why this is still a sidecar decision, not a platform decision:

- The vector layer is intentionally replaceable.
- The product requirements already state that embeddings are rebuildable and not source of truth.
- The vector ecosystem is changing quickly; committing the core archive to any vector engine would be a mistake.

Operational rule:

- The system must be able to delete and rebuild the entire vector sidecar from canonical SQLite data.

### 3.5 Heavy Analytics: DuckDB Deferred

DuckDB is not selected as a day-one required database, but it remains the leading analytics sidecar candidate if later needed.

Why not day one:

- The primary archive workload is not an OLAP workload.
- The current milestone order prioritizes Archive and Recall over advanced analytics.
- Introducing a second always-on derived database too early adds complexity without improving the core trust model.

When DuckDB becomes justified:

- long-window comparative analytics become expensive in SQLite
- the team needs repeated heavy ad-hoc analysis over large derived datasets
- insight generation benefits from separate OLAP storage and execution

If added later, DuckDB must remain:

- optional
- derived
- rebuildable
- non-authoritative

## 4. Evaluated Alternatives

### 4.1 PostgreSQL + pgvector

Assessment:

- Maturity: very high
- Feature completeness: very high
- Ecosystem / community: very strong
- Fit for this product: poor

Why not selected:

- PostgreSQL is fundamentally a client/server system.
- It introduces service lifecycle, operational coupling, and backup/recovery expectations that do not fit a local-first single-user desktop archive.
- `pgvector` is strong and widely adopted, but it solves the wrong problem shape for this product.

Verdict:

- Excellent technology
- Wrong center of gravity

### 4.2 libSQL / Turso

Assessment:

- Maturity: meaningful but strategically in motion
- Feature completeness: strong
- Ecosystem / community: growing
- Fit for canonical archive: medium at best

Why not selected:

- It is not the most conservative choice for a 20+ year local archive.
- The product and ecosystem emphasis are increasingly tied to replication/sync and a broader Turso platform direction.
- For this product, plain SQLite provides the simpler and more durable long-term baseline.

Verdict:

- Serious alternative
- Not the safest canonical choice

### 4.3 DuckDB As Primary Database

Assessment:

- Maturity: high for analytics
- Feature completeness: high for OLAP
- Ecosystem / community: very strong momentum
- Fit for canonical archive: poor

Why not selected:

- DuckDB is optimized for analytics and bulk data workflows, not as the canonical archive for many years of small transactional ingest plus rollback semantics.
- Its vector extension is still not mature enough to anchor production semantic retrieval for this product.

Verdict:

- Best analytics sidecar candidate
- Wrong primary database

### 4.4 Qdrant / Qdrant Edge

Assessment:

- Maturity: high for server Qdrant, lower for Edge
- Feature completeness: strong
- Ecosystem / community: strong
- Fit for desktop local vector sidecar: promising

Why not selected as the current final vector choice:

- Qdrant server is better suited to service-style deployment than this product needs.
- Qdrant Edge is especially interesting because it is embedded and local, but it is still officially beta.
- For a long-lived product, beta infrastructure should not become the committed default when a workable embedded alternative exists.

Verdict:

- Keep as the strongest future replacement candidate for the vector sidecar
- Do not standardize on it today

### 4.5 Chroma

Assessment:

- Maturity: real product, but younger and more deployment-layered
- Feature completeness: broad
- Ecosystem / community: visible and active
- Fit for long-horizon local archive product: weak

Why not selected:

- Chroma's architecture and deployment story are more favorable for experimentation, local prototyping, or service-oriented deployments than for a long-horizon personal archive substrate.
- It does not offer a compelling reason to sit in the core path of this product versus the simpler layered design above.

Verdict:

- Useful in other product shapes
- Not the right choice here

### 4.6 sqlite-vec / SQLite Vec1

Assessment:

- Maturity: early
- Feature completeness: incomplete
- Ecosystem / community: promising
- Fit for future experiments: high

Why not selected:

- The space is promising, but the maturity is not yet sufficient for this product's default vector strategy.
- The correct move is to keep watching this area, not to commit the product to it now.

Verdict:

- Track closely
- Do not standardize yet

## 5. Community And Maturity Readout

The community signal is clear when grouped by role rather than hype:

- `SQLite` and `PostgreSQL` are the mature infrastructure class.
- `SQLCipher` is the mature encryption layer for SQLite-style local apps.
- `DuckDB` is the mature modern embedded analytics engine.
- `pgvector` is mature, but attached to the wrong operational model for this product.
- `Qdrant`, `LanceDB`, and `Chroma` are all meaningful vector products, but this ecosystem is still changing quickly.
- `Qdrant Edge`, `sqlite-vec`, and `Vec1` are especially interesting because they move toward embedded local vector retrieval, but they are not yet the conservative long-term default.

This leads to a stable split:

- mature stack for truth: SQLite + SQLCipher + FTS5
- flexible stack for AI-derived retrieval: replaceable vector sidecar

## 6. Architecture Rules Going Forward

The following rules are part of the decision.

### 6.1 Source Of Truth Boundary

Only SQLite may hold canonical source-of-truth archive data.

This includes:

- visits
- URL metadata versions
- raw row versions
- import and backup runs
- rollback state
- schema migration state
- recall aggregates

This explicitly excludes:

- embeddings
- ANN indexes
- topic clusters
- thread assignments
- generated summaries
- rerank caches

### 6.2 Rebuildability

All AI-derived state must be deletable and rebuildable from canonical SQLite data.

The product must be able to:

- drop the vector sidecar
- rebuild embeddings
- rebuild vector indexes
- rebuild topic and thread derived state

without losing archive truth.

### 6.3 Adapter Boundary

The vector layer must be hidden behind a storage adapter.

The application must not hard-code LanceDB-specific assumptions into:

- canonical schema
- audit model
- rollback model
- import semantics

This preserves the option to move later to Qdrant Edge, a future SQLite-native ANN extension, or another embedded vector engine.

### 6.4 Milestone Discipline

- M1 and M2 should ship without any required vector sidecar.
- M3 may introduce the LanceDB sidecar for semantic search.
- DuckDB should only appear after concrete benchmark pain justifies it.

## 7. Revisit Triggers

This decision should be revisited only if one of the following becomes true:

- LanceDB becomes a poor operational fit under real-world local datasets.
- Qdrant Edge reaches clear production maturity and demonstrates a better embedded Rust integration story.
- SQLite-native ANN options become production-grade and materially simplify the stack.
- Recall and insights workloads prove that SQLite alone is insufficient for derived analytics.
- Product direction changes from local-first desktop archive to a multi-user or service-hosted system.

## 8. Final Decision Statement

The next-generation Browser History Vault should standardize on:

- `SQLite` as the canonical archive database
- `SQLCipher` as the encryption layer
- `SQLite FTS5` as the lexical recall layer
- `LanceDB` as the replaceable vector sidecar for semantic retrieval
- `DuckDB` only as a future optional analytics sidecar, never as source of truth

This decision maximizes long-term durability, local ownership, auditability, and architectural flexibility while keeping AI capabilities powerful but non-authoritative.

## 9. References

Primary references consulted during this decision:

- `docs/vision-and-requirements.md`
- `docs/數據庫選型意見.md`
- `docs/cto-feasibility-report-2026-04-05.md`
- `docs/longevity-capacity-analysis-2026-04-05.md`
- SQLite Long Term Support: <https://sqlite.org/lts.html>
- SQLite Limits: <https://sqlite.org/limits.html>
- SQLite FTS5: <https://sqlite.org/fts5.html>
- SQLite Backup API: <https://sqlite.org/backup.html>
- SQLite Session Extension: <https://sqlite.org/sessionintro.html>
- SQLCipher Design: <https://www.zetetic.net/sqlcipher/design/>
- PostgreSQL Architecture: <https://www.postgresql.org/docs/current/tutorial-arch.html>
- pgvector: <https://github.com/pgvector/pgvector>
- DuckDB Concurrency: <https://duckdb.org/docs/current/connect/concurrency>
- DuckDB SQLite Extension: <https://duckdb.org/docs/current/core_extensions/sqlite>
- DuckDB VSS Extension: <https://duckdb.org/docs/current/core_extensions/vss>
- Chroma Architecture Overview: <https://docs.trychroma.com/reference/architecture/overview>
- Qdrant Storage: <https://qdrant.tech/documentation/storage/>
- Qdrant Edge: <https://qdrant.tech/documentation/edge/>
- LanceDB Documentation: <https://docs.lancedb.com/>
- LanceDB Vector Indexes: <https://docs.lancedb.com/indexing/vector-index>
- SQLite Vec1: <https://sqlite.org/vec1/doc/trunk/doc/vec1.md>
- sqlite-vec: <https://alexgarcia.xyz/sqlite-vec/>
