# Browser History Backup Requirements Document

Status: Draft  
Last updated: 2026-04-05

## 1. Document Purpose

This document defines what `Browser History Backup` is, what user problems it solves, which requirements belong to the product, and which user stories must be supported by the redesigned desktop app.

This document is intentionally rewritten from the demand side. It does not inherit the structure, assumptions, or interaction patterns of earlier project documents or previous UI drafts.

## 2. Product Definition

`Browser History Backup` is a local-first desktop application for building a long-term, user-controlled archive of browser history.

It is not only a history viewer. It is a:

- browser history backup system
- local history vault
- audit and provenance tool
- import/export and recovery tool
- optional analysis and AI-assisted memory tool

The product exists to solve a simple core problem:

- mainstream browsers do not preserve a complete, user-owned history forever
- users cannot easily audit what was collected, when it was collected, or how it was transformed
- users cannot safely review, revert, deduplicate, analyze, and export their historical browsing data over many years

## 3. Product Vision

The product should feel like a trustworthy research-grade personal archive, not like a casual browser companion and not like a generic AI wrapper.

The app should help the user answer questions such as:

- What browser history data do I actually own?
- What has been backed up, imported, skipped, merged, or changed?
- Where is my data stored on disk?
- Can I trust this archive twenty years from now?
- Can I inspect or manually reproduce what the app is doing?
- Can I recover from a bad import or a bad automation setup?
- Can I search, export, and analyze my browsing history without giving it to a cloud service?

## 4. Product Positioning

The product should be designed around three layers.

### 4.1 Core Vault Layer

The foundation of the app is a durable, local, auditable browser history archive.

This layer includes:

- source discovery
- profile selection
- incremental backup
- import
- deduplication
- canonical storage
- encryption
- export
- rollback
- diagnostics

### 4.2 Trust and Operations Layer

The second layer explains and controls every sensitive or system-level operation.

This layer includes:

- step-by-step previews
- manual alternatives to automatic actions
- scheduler setup guidance
- audit logs
- manifests
- versioning and rollback
- remote backup configuration

### 4.3 Optional Intelligence Layer

The third layer adds search, analysis, and AI features on top of the vault.

This layer must remain optional and disabled by default.

This layer includes:

- semantic search
- agentic Q&A over archived history
- trend and task analysis
- embeddings
- LLM provider configuration
- MCP and skill integration

## 5. Product Principles

The redesigned product must follow these principles.

### 5.1 Local First

- The app stores and processes user data locally by default.
- Core backup, search, export, and audit workflows must work without a cloud account.
- AI features may use local or remote providers, but they are optional.

### 5.2 Trust Through Visibility

- The app must show what it is about to do, what it did, and why it did it.
- Sensitive operations must never feel magical or hidden.
- Users must be able to inspect commands, files, destinations, and generated artifacts.

### 5.3 Preview, Manual, Apply

Every system-level workflow must support three modes:

- `Preview`: show the planned steps, affected files, commands, permissions, and rollback implications
- `Manual`: let the user perform each step themselves with clear guidance and copyable commands
- `Apply`: let the app perform the steps only after explicit confirmation

### 5.4 Reversible by Design

- Imports, backups, migrations, encryption changes, and cleanup tasks must be attributable to a specific run or batch.
- The user must be able to review and roll back import batches or other reversible changes when possible.
- The app must help the user identify dirty data before and after import.

### 5.5 Evidence First

- The app should present concrete records, runs, counts, and provenance before generating summaries or AI explanations.
- AI must sit on top of the archive, not replace the archive.

### 5.6 Clear State and Clear Selection

- Selection UI must make the current state obvious.
- Multi-select items such as browser profiles must use explicit checkboxes or equally unambiguous controls.
- The user must always be able to tell whether a source is selected, unselected, active, inactive, healthy, or needs attention.

### 5.7 Long-Term Durability

- The system must be designed for 20+ years of data retention.
- Data format evolution, schema migration, and exportability must be first-class concerns.

## 6. Target Users

Primary users:

- privacy-conscious users who want a durable local archive of their browsing history
- technical users who want auditability, inspectability, and control
- researchers, writers, developers, and knowledge workers who revisit old browsing trails
- users who want to search or analyze long-term browsing behavior

Secondary users:

- power users migrating between browsers or machines
- users importing history from Google Takeout or other archive sources
- users who want optional AI-assisted recall or analysis over local data

## 7. Core User Jobs

The app must help users do the following jobs well.

- Preserve browser history beyond browser retention windows
- Understand exactly what data was collected and from where
- Choose which browsers and profiles to include
- Keep backups running over time without relying on the app staying open
- Inspect and manually reproduce automation and permission-related setup
- Keep the vault encrypted if desired
- Search, browse, filter, and export history later
- Import older history archives and Takeout data
- Detect duplicate or dirty imported records
- Revert problematic imports or recover from mistakes
- Back up the archive to remote storage such as S3-compatible services
- Optionally run semantic search, agentic Q&A, and insights over the archive

## 8. Scope Definition

### 8.1 In Scope

- local desktop GUI application
- browser history backup and archival
- multi-browser and multi-profile support
- cross-platform product direction for macOS, Windows, and Linux
- step-by-step guided system workflows
- optional automation via native OS schedulers
- import from local browser data sources
- import from Google Takeout and similar archive sources
- export to structured and human-readable formats
- local vault encryption
- rollback and audit history for imports and change-producing operations
- remote backup to S3-compatible storage
- i18n for English, Simplified Chinese, and Traditional Chinese
- optional AI, embeddings, insights, MCP, and skill integration

### 8.2 Out of Scope

- turning the product into a cloud sync service
- mandatory account creation
- hidden background automation with no user review path
- modifying the live browser database in place
- default-on AI features
- creepy or highly sensitive inference as a default feature

## 9. Product Mental Model

The product should teach a small set of concepts clearly.

- `Source`: a browser installation, profile, archive file, or Takeout package
- `Vault`: the app-managed long-term archive
- `Run`: one execution of backup, import, export, integrity check, or analysis
- `Batch`: a set of imported records that can be reviewed together
- `Artifact`: a generated file, config file, manifest, export, or scheduler asset
- `Ledger`: the audit view of runs, changes, warnings, and provenance
- `Rollback Point`: a reversible boundary tied to a run or import batch

No major screen should blur these concepts together.

## 10. Launch Scope vs Target Scope

The app name is `Browser History Backup`, not `Chrome History Backup`. Product design must reflect that broader direction.

### 10.1 Launch Scope

The first complete end-to-end experience should prioritize:

- macOS
- Chromium-family browsers
- local browser profile import
- Google Takeout import
- durable vault management
- auditability
- export
- rollback
- optional encryption

### 10.2 Target Scope

The long-term target should include:

- Chromium-family browsers across macOS, Windows, and Linux
- Firefox support
- Safari support where platform constraints allow
- additional archive import paths
- richer remote backup and restore workflows

## 11. Functional Requirements

### 11.1 Desktop App and Platform Requirements

- The product must be delivered as a desktop application, not only as scripts or a local web page.
- The primary interaction surface must be a GUI.
- The app should still expose manual commands and inspectable artifacts for power users.
- The product architecture should remain compatible with macOS, Windows, and Linux.

### 11.2 Onboarding and Setup

The app must provide a first-run setup flow that explains:

- what the product is
- what data it can access
- what it cannot recover once deleted by the source browser
- where the vault will live
- whether encryption is enabled
- how automation works
- how to proceed manually instead of automatically

The first-run experience must not assume the user understands:

- browser profile locations
- OS schedulers
- encryption tradeoffs
- import batch semantics
- AI provider setup

### 11.3 Source Discovery and Profile Selection

- The app must detect supported local browsers and available profiles.
- The user must be able to select one or more profiles using unambiguous multi-select controls.
- Selected state must be visually obvious.
- Deselected state must be visually obvious.
- The UI should include simple motion feedback so the user can tell whether a click added or removed a profile.
- The UI must summarize how many profiles are selected and which ones they are.
- The user must be able to inspect detected source paths.

### 11.4 Backup and Incremental Capture

- The vault must support incremental backup.
- Incremental backup must avoid creating duplicate history records when the same records are seen again.
- The vault should retain as much relevant source metadata as practical, including titles, timestamps, profile identity, visit relationships, browser-specific fields, and other provenance-bearing fields that can support future migration or export.
- The system must preserve sufficient raw and derived data for long-term auditability and future migration.
- The system must support very large retention windows, including 20+ years of accumulated data.
- The system must retain enough metadata to keep provenance and future export possible.

### 11.5 Deduplication and Canonicalization

- The system must detect repeated history records across repeated backups and repeated imports.
- The system must merge, skip, or version duplicates rather than silently duplicating them.
- The deduplication strategy must be stable enough for incremental backup use cases.
- The app must expose when records were skipped, merged, or considered equivalent.

### 11.6 Audit Ledger and Provenance

- Every backup, import, export, rollback, scheduler change, migration, and integrity check must create a run record.
- Run records must show timestamps, source, status, counts, warnings, and generated artifacts.
- The user must be able to trace a history record back to the run or import batch that introduced it.
- The ledger should make it obvious which actions changed data and which actions were read-only.
- The product should produce human-inspectable audit artifacts that can be tracked in version control systems such as Git, without requiring the vault database itself to be treated as a Git payload.

### 11.7 Import from Local Browser Sources

When importing from installed browsers, the app must provide a guided step-by-step workflow that explains:

- what file or database is being read
- why the step is necessary
- whether the app will perform it automatically or the user can do it manually
- what command or artifact is involved
- what to expect on success or failure

The same workflow must support:

- preview mode
- manual mode
- apply mode

### 11.8 Import from Google Takeout and Other Archives

- The app must support user-initiated import from Google Takeout browser history archives.
- The app must provide a dry-run or preview phase before the user commits the import.
- The user must be able to review what will be imported.
- The app must identify unsupported or suspicious files and surface them instead of silently ignoring them.
- Each import must be associated with an import batch.
- The user must be able to review imported records by batch.
- The user must be able to revert a problematic import batch when feasible.

### 11.9 Versioning and Rollback

- The product must provide version-aware history management at the run or batch level.
- If a user imports dirty data, they must have a way to identify and roll back that import.
- The app must clearly separate archive facts from user actions that changed the archive.
- Rollback operations must themselves be auditable.
- The UI must explain the consequences of rollback before execution.

### 11.10 Scheduler and Automation

- The product must support scheduled backup using native OS schedulers rather than requiring the app to stay open forever.
- Scheduling must support the due-aware behavior requested by the user: if the machine has been off, the task should run after boot or login once the interval has elapsed.
- Users must be able to configure automation from inside the GUI.
- Users must also be able to perform the same setup manually.
- The app must provide step-by-step instructions, copyable commands, generated scheduler artifacts, and explanations of why each step exists.
- Scheduler setup and changes must appear in the audit ledger.

### 11.11 Security and Encryption

- The vault must support encrypted and unencrypted modes.
- Encryption should apply at least to the history-containing database.
- The UI must clearly warn users that forgetting the password may make the data permanently unrecoverable.
- The app must strongly recommend storing the password in a password manager.
- The user must be allowed to choose plaintext mode with an explicit warning.
- The product must support password change and rekey flows.
- The app must explain whether convenience unlock is stored in an OS keyring and how that affects security.

### 11.12 Remote Backup

- The product must support backing up the vault or designated backup artifacts to S3-compatible remote storage.
- Remote backup must be configurable from the GUI.
- Remote backup workflows must also support preview, manual, and apply paths.
- The user must understand what is being uploaded, whether it is encrypted, and how restore would work.
- Remote backup runs must be auditable like local runs.

### 11.13 History Explorer

- The user must be able to browse archived history in the app.
- The explorer must support filtering by date, browser, profile, domain, source, and import batch or run when relevant.
- The explorer must make provenance visible.
- The explorer must make deduplicated or merged state understandable.
- The explorer must be useful for both recent and very old data.

### 11.14 Export

- The app must support export to structured and human-readable formats.
- Supported export targets should include database-like structured output and formats such as JSON, JSONL, HTML, Markdown, and plain text.
- Export results must be attributable to a query or run.
- Export should preserve provenance where practical.
- Export can be local or included in a remote backup workflow.

### 11.15 Data Location and Inspectability

- The app must show the exact path where the vault data is stored.
- The app must allow the user to open that directory from the UI.
- The app must display app version and a short git commit identifier in the UI.
- The app must surface enough system information for debugging and support.

### 11.16 Internationalization

- The app must support English, Simplified Chinese, and Traditional Chinese.
- The app must detect the user device language on first launch and choose a default language accordingly.
- The user must be able to change language from settings.

### 11.17 AI and Analysis Features

AI features are optional product extensions and must remain off by default.

These features include:

- embedding-powered semantic search
- LLM-assisted question answering over archived history
- trend and thread analysis
- agentic retrieval over the local archive
- MCP and skill access to the archive when manually enabled by the user

### 11.18 AI Provider Configuration

- The app must allow users to configure multiple providers for LLM and embedding workloads.
- A provider is defined by request format, base URL, API key or other secret, model list, and model-specific settings.
- The product must support at least OpenAI-style, Anthropic-style, and Google-style request formats.
- Users must be able to add multiple providers of the same request format.
- Users must be able to edit the base URL.
- Providers must be individually enabled or disabled.
- The app must support local endpoints such as Ollama and LM Studio as configuration targets when compatible.

### 11.19 AI Product Direction

The AI layer should favor embedding-first, LLM-last design.

This means:

- use embeddings for retrieval, grouping, semantic similarity, and recall
- use LLMs mainly for naming, summarization, explanation, and on-demand Q&A
- avoid making heavy LLM inference the backbone of core archive behavior

### 11.20 Insights and Analysis Scope

The first useful analysis features should focus on:

- semantic recall
- task or thread reconstruction
- topic timeline and trend shifts
- revisit and resurfacing signals
- query reformulation patterns
- workflow or source-role maps
- contrastive summaries over time windows

The product should avoid leading with:

- personality judgments
- highly sensitive inference
- opaque scores with no evidence

## 12. UX Requirements for Redesign

The redesign should treat product clarity as a hard requirement, not a polish task.

### 12.1 Primary Information Architecture

The product should be structured around these top-level areas:

- Home
- Sources
- Backup and Automation
- Vault and Security
- Explorer
- Imports
- Exports and Remote Backup
- Ledger and Diagnostics
- AI and Insights
- Settings

### 12.2 Home Screen Requirements

The home screen should answer five questions immediately:

- Is my vault healthy?
- When did the last successful backup run?
- Which sources are connected?
- Is automation configured?
- What can I do next?

### 12.3 Step-by-Step Workflow Requirements

For source import, permissions-related setup, automation, remote backup, and other sensitive operations:

- the UI must present steps in order
- each step must explain purpose
- each step must show whether it is automatic or manual
- each step must provide commands or artifacts when manual action is possible
- each step must have a clear completed, pending, failed, or skipped state

### 12.4 Selection and Feedback Requirements

- Profile selection must use explicit check-style interaction or an equally clear alternative.
- The user must get immediate visual confirmation when a profile becomes selected or deselected.
- Motion should reinforce state change without adding noise.

### 12.5 Noise Reduction

- The UI must remove decorative or template-like noise that obscures the product’s mental model.
- Visual design should support auditability, evidence review, and operational clarity.
- Optional AI surfaces must not overpower the core vault experience.

## 13. User Stories

### 13.1 Core Archive Stories

- As a user, I want to back up my browser history locally so that I can keep it far longer than browsers normally retain it.
- As a user, I want to choose which browsers and profiles are included so that my archive reflects my intent.
- As a user, I want repeated backups to avoid duplicating old history so that my archive stays clean.
- As a user, I want to keep provenance for archived records so that I can trust where they came from.
- As a user, I want to inspect the data store location so that I know what the app owns on disk.

### 13.2 Trust and Automation Stories

- As a cautious user, I want the app to explain every automation-related step so that I can decide whether to let it act automatically.
- As a cautious user, I want a manual path for scheduler setup so that I can perform and audit it myself.
- As a user, I want backup to run after boot or login when due so that I do not lose backups just because my machine was off.
- As a user, I want scheduler artifacts and commands to be visible so that I can reproduce or remove them later.

### 13.3 Security Stories

- As a security-conscious user, I want to encrypt the archive so that local history is not stored as plaintext.
- As a user, I want a clear warning about password loss so that I understand the risk before enabling encryption.
- As a user, I want to rekey the vault later so that I am not locked into my first password forever.
- As a user, I want the option to use plaintext mode so that I can make my own tradeoff.

### 13.4 Import and Rollback Stories

- As a user, I want to import Google Takeout history so that I can bring older data into the vault.
- As a user, I want to preview an import before committing it so that I can catch suspicious data early.
- As a user, I want each import to be tracked as a distinct batch so that I can review its impact.
- As a user, I want to revert a bad import so that dirty data does not permanently contaminate the vault.

### 13.5 Explorer and Export Stories

- As a user, I want to search and filter long-term history so that I can rediscover old browsing trails.
- As a user, I want to export results in formats I can inspect or reuse so that my archive is not trapped in the app.
- As a user, I want to back up my vault or artifacts to S3-compatible storage so that I have off-device protection.

### 13.6 AI and Insight Stories

- As a user, I want semantic search over my history so that I can find things even when I do not remember exact titles.
- As a user, I want to ask questions about my past browsing so that I can reconstruct research trails.
- As a user, I want trend and thread insights so that I can understand what I have been working on over time.
- As a user, I want AI to be optional and disabled by default so that the core app still feels safe and local.
- As a power user, I want to expose the archive through MCP or skills only when I explicitly enable it.

### 13.7 Internationalization and Transparency Stories

- As a user, I want the app to open in my language by default when possible.
- As a user, I want to switch languages later from settings.
- As a user, I want to see the app version and short git commit so that I know exactly what build I am using.

## 14. Non-Functional Requirements

### 14.1 Reliability

- The product must be designed for long-term data retention.
- The system must survive repeated incremental runs without accumulating uncontrolled duplication.
- The app must tolerate future browser schema changes better than a one-off scraper.

### 14.2 Performance

- Common browsing and search workflows should remain responsive on large local archives.
- Background analysis and AI indexing should not block the main app experience.

### 14.3 Auditability

- Core archive operations must be inspectable.
- The user must be able to understand what changed after each write operation.

### 14.4 Portability

- The archive must remain exportable to durable, inspectable formats.
- The product should not trap user data in a proprietary black box.

### 14.5 Accessibility and Clarity

- UI state must be understandable without guesswork.
- Critical warnings must be readable and hard to miss.
- Sensitive actions must require deliberate confirmation.

## 15. Engineering and Delivery Requirements

These are not end-user features, but they are still project requirements that affect trust and sustainability.

- The project should be open-source under GPLv3.
- The repository should include a current README and contribution guide.
- The repository should include CI, release automation, linting, type checking, test coverage, and mutation testing where required by the project quality bar.
- Rust-side quality expectations include complete linting and the target of 100% test coverage with integration and end-to-end coverage where practical.
- JavaScript and frontend quality expectations include linting, type checking, tests, and equivalent rigor.
- Release automation should produce installable artifacts for supported operating systems.
- The repository should expose build and run instructions for both source development and packaged app workflows.

## 16. Priority Order for Redesign

To avoid repeating the earlier UX failure, the redesign should treat features in this order.

### 16.1 Must Be Crystal Clear First

- product purpose
- source selection
- vault location
- backup status
- automation setup
- import review and rollback
- security mode
- ledger visibility

### 16.2 Then Make Power Features Understandable

- export
- remote backup
- diagnostics
- deduplication details
- versioning and rollback detail

### 16.3 Then Layer on Optional Intelligence

- semantic search
- AI assistant
- insights
- MCP and skill integration

If a screen tries to explain all three layers equally at once, the design is probably wrong.

## 17. Product Summary in One Sentence

`Browser History Backup` is a local, auditable, long-term browser history vault that helps users preserve, inspect, search, export, recover, and optionally analyze their browsing history with clear step-by-step control over every sensitive operation.
