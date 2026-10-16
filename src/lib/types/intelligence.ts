/**
 * This module defines typed front-end contracts for AI providers, Core Intelligence, queue state, and runtime review surfaces.
 *
 * Why this file exists:
 * - The UI reads these shapes as its desktop and preview contract, so unclear names here ripple through every consumer.
 * - If you need to know what a route or helper expects from the backend, this is often the fastest file to open first.
 *
 * Main declarations:
 * - `EnrichmentPluginState`
 * - `EnrichmentSettings`
 * - `DeterministicModuleState`
 * - `DeterministicSettings`
 * - `AiRequestFormat`
 * - `AiProviderPurpose`
 * - `AiProviderConfig`
 * - `EnrichmentPluginPreference`
 * - `AiSettings`
 * - `AiIndexStatus`
 *
 * Source-of-truth notes:
 * - Data shapes should stay aligned with the accepted architecture and feature docs rather than ad-hoc page assumptions.
 * - Prefer additive, explicit fields over ambiguous catch-all objects so the trust surface stays auditable.
 */

import type { SearchCoverage } from '../backend-client/search'
import type { ScheduleGeneratedFile } from './schedule'

/**
 * Captures the state shape used by `EnrichmentPlugin`.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface EnrichmentPluginState {
  id: string
  enabled: boolean
  /**
   * Internal runtime marker used for diagnostics and storage diffing.
   * The primary UI should not surface this value in normal review chrome.
   */
  version: string
}

/**
 * Defines the typed shape for enrichment settings.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface EnrichmentSettings {
  plugins: EnrichmentPluginState[]
}

/**
 * Captures the state shape used by `DeterministicModule`.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface DeterministicModuleState {
  id: string
  enabled: boolean
  /**
   * Internal runtime marker used for diagnostics and storage diffing.
   * The primary UI should not surface this value in normal review chrome.
   */
  version: string
}

/**
 * Defines the typed shape for deterministic settings.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface DeterministicSettings {
  modules: DeterministicModuleState[]
}

/**
 * Defines the type-level contract for ai request format.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export type AiRequestFormat =
  | 'openai'
  | 'anthropic'
  | 'google'
  | 'ollama'
  | 'lm-studio'

/**
 * Defines the type-level contract for ai provider purpose.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export type AiProviderPurpose = 'llm' | 'embedding'

/**
 * Represents persisted configuration for ai provider.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AiProviderConfig {
  id: string
  name: string
  purpose: AiProviderPurpose
  requestFormat: AiRequestFormat
  enabled: boolean
  baseUrl?: string | null
  apiKeySaved: boolean
  defaultModel: string
  modelCatalog: string[]
  dimensions?: number | null
  notes?: string | null
}

/**
 * Defines the typed shape for enrichment plugin preference.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface EnrichmentPluginPreference {
  pluginId: string
  enabled: boolean
}

/**
 * Mirrors the Rust `BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID`; the always-on on-device embedding tier.
 */
export const BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID = 'static-in-app'

/**
 * Defines the typed shape for ai settings.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AiSettings {
  enabled: boolean
  assistantEnabled: boolean
  semanticIndexEnabled: boolean
  mcpEnabled: boolean
  skillEnabled: boolean
  autoIndexAfterBackup: boolean
  jobQueuePaused: boolean
  jobQueueConcurrency: number
  enrichmentEnabled: boolean
  enrichmentPlugins: EnrichmentPluginPreference[]
  /**
   * Master switch for site CONTENT fetching (W-ENRICH-1, 06 §2a). Mirrors the
   * Rust `content_fetch_enabled` (`#[serde(default)]`, hard-default-OFF) and is
   * INDEPENDENT of `enrichmentEnabled` (which only governs the offline title
   * plugin). Optional on the TS side so older snapshots without the field still
   * type-check; treat absent/false as "off".
   */
  contentFetchEnabled?: boolean
  /** Per-extractor content-fetch toggles (W-ENRICH-1). Optional for older snapshots. */
  contentFetchExtractors?: ContentFetchExtractorPreference[]
  /** Per-domain content-fetch allow/block rules (W-ENRICH-1). Optional for older snapshots. */
  contentFetchDomains?: ContentFetchDomainRule[]
  llmProviderId?: string | null
  embeddingProviderId?: string | null
  retrievalTopK: number
  assistantSystemPrompt: string
  llmProviders: AiProviderConfig[]
  embeddingProviders: AiProviderConfig[]
  /**
   * Hybrid-search tuning knobs (W-AI-6). Mirror the Rust `AiSettings` fields (all `#[serde(default)]`,
   * clamped on load), so a settings UI can bind them later (W-AI-9). Optional here so older snapshots
   * without the fields still type-check; absent means "use the backend default".
   *
   * - `hybridRrfK` — Reciprocal Rank Fusion constant `k` (default 60; `>= 1`).
   * - `lexicalWeight` / `semanticWeight` — per-list fusion weights (default 1.0 each; `[0, 100]`).
   * - `starredBoost` — BOUNDED additive boost on a starred result's normalized score (default 0.15;
   *   `[0, 0.5]`) so favorites rank higher without becoming a bookmark list.
   */
  hybridRrfK?: number
  lexicalWeight?: number
  semanticWeight?: number
  starredBoost?: number
}
/**
 * Represents a read model or status snapshot for ai index.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
/**
 * One stable, locale-independent index-health warning CODE. Mirrors the Rust
 * `AiIndexWarning` (`#[serde(tag = "code", rename_all = "camelCase")]`). Interpolated variants carry
 * their params STRUCTURALLY (provider id / name) so the FE composes the localized string; `buildFailed`
 * carries the opaque transport reason (no fixed vocabulary). The FE maps ALL variants to localized
 * copy and NEVER matches on the English `AiIndexStatus.warning` sentence.
 */
export type AiIndexWarning =
  | { code: 'archiveNotInitialized' }
  | { code: 'noEmbeddingProvider' }
  | { code: 'embeddingProviderMissing'; providerId: string }
  | { code: 'embeddingProviderDisabled'; providerName: string }
  | { code: 'embeddingProviderNoApiKey'; providerName: string }
  | { code: 'embeddingProviderNoModel'; providerName: string }
  | { code: 'indexNotBuilt' }
  | { code: 'indexStale'; reason: AiSemanticStaleness }
  | { code: 'buildFailed'; reason: string }
  /**
   * The semantic vector store is empty even though the SQLite metadata records rows as indexed.
   * This typically means the embedding provider produced no output (wrong model, bad config, or
   * a provider that returned success without writing vectors). Mirrors the Rust
   * `AiIndexWarning::IndexVectorsMissing`.
   */
  | { code: 'indexVectorsMissing' }

/**
 * Static in-app embedding tier status (additive on AiIndexStatus). Mirrors the Rust
 * `StaticEmbeddingStatus` (camelCase serde, `#[serde(default, skip_serializing_if)]`).
 * Always present when the static-in-app provider is registered; absent on older backends.
 */
export interface StaticEmbeddingStatus {
  /** Always `"static-in-app"`. */
  providerId: string
  /** Hugging Face repo identifier for the bundled model. */
  modelRepo: string
  /** Whether the model weights have been downloaded to the local cache. */
  modelDownloaded: boolean
  /** Whether this provider is currently the active embedding selection. */
  selected: boolean
  /** Whether this provider is the system default when no explicit selection exists. */
  isDefault: boolean
  /**
   * The static model's REAL embedding dimension (256 for `potion-multilingual-128M`). Mirrors the Rust
   * `dimensions` (`#[serde(default)]`); absent on older backends, treat as unknown (0). Use THIS for
   * the Settings tier card instead of inferring 1536 from an absent `AiProviderConfig.dimensions`.
   */
  dimensions?: number
  /**
   * Total on-disk byte size of the model's files once present + verified, else 0 (pre-download the UI
   * shows a static estimate). Mirrors the Rust `modelSizeBytes` (`#[serde(default)]`); absent on older
   * backends.
   */
  modelSizeBytes?: number
}

/**
 * One in-app embedding model download progress event, delivered on the
 * `pathkeep://model-download-progress` Tauri channel. Mirrors the Rust
 * `ModelDownloadProgressEvent` (`#[serde(tag = "kind", rename_all = "camelCase")]`).
 *
 * The in-app model download emits on this channel; the stream is per-file (no upfront file count and `totalBytes` may be 0 when unknown),
 * so the UI shows honest per-file / indeterminate progress, never a fabricated percent.
 * Exactly one terminal `done`/`error` always follows so a subscriber never hangs.
 *
 * `fileStarted` now carries the REAL HTTP content-length (0 only when the server omits it), and
 * `fileProgress` reports throttled byte progress (~every 1 MB plus a final 100%) so the UI renders a
 * real moving bar instead of a near-empty per-file indicator while the 90 MB safetensors streams.
 */
export type ModelDownloadProgressEvent =
  | { kind: 'fileStarted'; file: string; totalBytes: number }
  | {
      kind: 'fileProgress'
      file: string
      downloadedBytes: number
      totalBytes: number
    }
  | { kind: 'fileFinished'; file: string }
  | { kind: 'done' }
  | { kind: 'error'; message: string }

export interface AiIndexStatus {
  enabled: boolean
  assistantEnabled: boolean
  mcpEnabled: boolean
  skillEnabled: boolean
  state: string
  ready: boolean
  indexedItems: number
  lastIndexedAt?: string | null
  llmProviderId?: string | null
  embeddingProviderId?: string | null
  queuePaused: boolean
  queueConcurrency: number
  queuedJobs: number
  runningJobs: number
  failedJobs: number
  recentJobs: AiQueueJob[]
  semanticSidecarBytes: number
  semanticMetadataBytes: number
  estimatedEmbeddingTokens: number
  /**
   * English warning prose, retained for backward compatibility. The user-facing Settings surface
   * reads `warningCode` instead and never renders this raw for a code it can localize.
   */
  warning?: string | null
  /**
   * Stable index-health warning CODE, additive (mirrors Rust
   * `#[serde(default, skip_serializing_if)]`). The FE maps ALL variants to localized copy.
   */
  warningCode?: AiIndexWarning | null
  /**
   * The REAL count of vectors written to the semantic sidecar (HNSW + SQLite metadata). Distinct
   * from `indexedItems` (which counts SQLite metadata rows): when `semanticVectorCount === 0` but
   * `indexedItems > 0` the embedding provider wrote metadata without producing any vectors, which
   * the backend reports as `state: "degraded"` + `IndexVectorsMissing`. Additive, mirrors Rust
   * `#[serde(default, skip_serializing_if)]`; absent on older backends, treat as unknown.
   */
  semanticVectorCount?: number | null
  /**
   * Status of the built-in static embedding tier. Additive, mirrors Rust
   * `#[serde(default, skip_serializing_if)]`; absent when the static provider is not registered.
   */
  staticEmbedding?: StaticEmbeddingStatus | null
}

/**
 * Represents a completed report that the UI can review after a run finishes.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AiProviderCapabilityReport {
  supportsChat: boolean
  supportsEmbeddings: boolean
  supportsStreaming: boolean
  supportsToolUse: boolean
  supportsStructuredOutput: boolean
}

/**
 * Serialized, UI-facing mirror of the in-engine LLM capabilities.
 *
 * Attached to a connection-test report for LLM providers (null for embedding providers). It
 * exposes the exact streaming/tool/structured/cache facts the chat transport relies on, plus
 * the known context window (`maxContextTokens` is null unless the transport reports one).
 */
export interface LlmProviderCapabilityReport {
  toolCall: boolean
  structuredOutput: boolean
  streaming: boolean
  promptCache: boolean
  maxContextTokens?: number | null
}

/**
 * Describes a request payload in this front-end contract.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AiProviderConnectionTestRequest {
  providerId: string
  purpose: AiProviderPurpose
}

/**
 * Which service to ask for its models: the form's values, saved or not. The
 * key is the one typed in the form, else the stored key of `providerId`.
 */
export interface AiProviderModelsRequest {
  purpose: AiProviderPurpose
  requestFormat: AiRequestFormat
  baseUrl: string | null
  providerId?: string | null
  apiKey?: string | null
  /**
   * Read only the list under the API root the service's requests go to
   * (saving a service checks it this way; no model is loaded).
   */
  apiRootOnly?: boolean
}

/** What a model is for, as the service says; `unknown` when it does not say. */
export type AiModelKind = 'chat' | 'embedding' | 'unknown'

/**
 * One model a service offers. `loadedContextLength` is how much it can read
 * as loaded now; `maxContextLength` the most it could be loaded with.
 */
export interface AiProviderModel {
  id: string
  kind: AiModelKind
  loaded: boolean | null
  loadedContextLength: number | null
  maxContextLength: number | null
}

/**
 * Represents a completed report that the UI can review after a run finishes.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AiProviderConnectionTestReport {
  providerId: string
  purpose: string
  model: string
  ok: boolean
  latencyMs: number
  capabilities: AiProviderCapabilityReport
  llmCapabilities?: LlmProviderCapabilityReport | null
  errorCode?: string | null
  actionHint?: string | null
  retryHint?: string | null
  warnings: string[]
  message: string
}

/**
 * Defines the typed shape for ai queue job.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AiQueueJob {
  id: number
  jobType: string
  state: string
  priority: number
  attempt: number
  maxAttempts: number
  runId?: number | null
  summary?: string | null
  queuedAt: string
  availableAt: string
  startedAt?: string | null
  finishedAt?: string | null
  heartbeatAt?: string | null
  errorCode?: string | null
  errorMessage?: string | null
  /**
   * Index-build progress parsed from the job's resumable cursor (Change 1). Populated ONLY for
   * index-build/index-clear jobs; absent (undefined) for assistant jobs and unparseable payloads.
   * Mirrors the Rust `AiQueueJob` (`#[serde(default, skip_serializing_if = "Option::is_none")]`).
   *
   * - `progressEmbedded` — vectors durably embedded so far (the cursor's `embedded_so_far`).
   * - `progressScanned` — the scan watermark: lowest `history_id` not yet processed (`next_history_id`).
   * - `progressScanTarget` — the determinate scan denominator (max candidate `history_id` captured at
   *   the build's true start; 0 = not yet known). `progressScanned / progressScanTarget` is an honest
   *   determinate bar that reaches ~100%.
   * - `progressEmbedTarget` — total candidate pages captured at the build's true start; absent or 0 =
   *   not yet known. `progressEmbedded / progressEmbedTarget` is the honest fill ratio preferred over
   *   the scan-cursor fallback.
   */
  progressEmbedded?: number
  progressScanned?: number
  progressScanTarget?: number
  progressEmbedTarget?: number
  /** Who paused it, when its state is `paused`. */
  pausedBy?: IndexBuildPause | null
}

/** One semantic index: the service it was made with and the model. */
export interface EmbeddingIndexRef {
  providerId: string
  model: string
}

/**
 * Who paused an index build: `user` (the pause button), `service` (the
 * embeddings service could not load the model or could not be reached; the
 * job's `errorMessage` has its own words), `queue` (all background work is
 * paused). A build paused by the user or the service waits for the user; one
 * paused with the queue goes on when the queue does.
 */
export type IndexBuildPause = 'user' | 'service' | 'queue'

/** The chosen model's index build, while it is queued, running or paused. */
export interface IndexBuildJob {
  jobId: number
  state: 'queued' | 'running' | 'paused'
  pausedBy?: IndexBuildPause | null
  /** The service's own words when `pausedBy` is `service`. */
  pauseMessage?: string | null
  /** Visits this build has indexed so far. */
  done: number
  /** Visits this build has to index; 0 until the build has counted them. */
  total: number
}

/**
 * Which index search reads and how far the chosen model's index is. Changing
 * the model does not build anything: search keeps reading the last complete
 * index until the user builds the chosen model's and it completes.
 */
export interface SemanticIndexState {
  /** The model chosen in Settings → AI; null when semantic search has none. */
  chosen: EmbeddingIndexRef | null
  /**
   * The index search reads now: the chosen model's once it is complete,
   * before that the most recent complete index whose service is still set up;
   * null when there is none.
   */
  serving: EmbeddingIndexRef | null
  /** The chosen model's index is complete and backups keep it up to date. */
  chosenComplete: boolean
  build: IndexBuildJob | null
}

/**
 * Represents a read model or status snapshot for ai queue.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AiQueueStatus {
  paused: boolean
  concurrency: number
  queued: number
  running: number
  failed: number
  /**
   * Queued/paused/stale count NARROWED to semantic-index jobs (index-build +
   * index-clear). The Smart-search build callout keys its phase off this so an
   * in-flight assistant chat job — counted in the aggregate `queued`/`running`
   * above — never reads as build progress.
   */
  indexQueued: number
  /**
   * Running count NARROWED to semantic-index jobs; the index-only companion to
   * `indexQueued`.
   */
  indexRunning: number
  recentJobs: AiQueueJob[]
  semanticIndex: SemanticIndexState
}

/**
 * Represents a read model or status snapshot for Core Intelligence runtime readiness.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface IntelligenceStatus {
  ready: boolean
  lastRunAt?: string | null
  runs: number
  cards: number
  topics: number
  threads: number
  queryGroups: number
  referencePages: number
  contentCoverage: number
  warning?: string | null
}

/**
 * Represents a read model or status snapshot for intelligence queue.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface IntelligenceQueueStatus {
  queued: number
  running: number
  succeeded: number
  failed: number
  cancelled: number
  lastActivityAt?: string | null
}

/**
 * Represents a read model or status snapshot for enrichment plugin.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface EnrichmentPluginStatus {
  pluginId: string
  sourceKind: string
  enabled: boolean
  storedRecords: number
  queuedJobs: number
  runningJobs: number
  failedJobs: number
  lastCompletedAt?: string | null
  lastError?: string | null
}

/**
 * Defines the typed shape for intelligence job overview.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface IntelligenceJobOverview {
  id: number
  jobType: string
  pluginId?: string | null
  state: string
  historyId?: number | null
  profileId?: string | null
  url?: string | null
  title?: string | null
  attempt: number
  createdAt: string
  startedAt?: string | null
  finishedAt?: string | null
  updatedAt: string
  heartbeatAt?: string | null
  progressLabel?: string | null
  progressDetail?: string | null
  progressCurrent?: number | null
  progressTotal?: number | null
  progressPercent?: number | null
  /** Diagnostic English prose; localize through `fallbackReasonCode`. */
  fallbackReason?: string | null
  /**
   * Stable `REBUILD_FALLBACK_*` code for `fallbackReason`; absent for job
   * artifacts written before code-ification.
   */
  fallbackReasonCode?: string | null
  lastError?: string | null
  retryable: boolean
  cancellable: boolean
}

/**
 * Represents a read model or status snapshot for deterministic module runtime.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface DeterministicModuleRuntimeStatus {
  moduleId: string
  enabled: boolean
  version: string
  status: string
  dependsOn: string[]
  derivedTables: string[]
  lastRunId?: number | null
  lastBuiltAt?: string | null
  lastInvalidatedAt?: string | null
  /** Diagnostic English prose; localize through `staleReasonCode`. */
  staleReason?: string | null
  /** Stable code for `staleReason`; absent when the prose is opaque. */
  staleReasonCode?: string | null
  /** Diagnostic English prose, index-aligned with `noteDetails`. */
  notes: string[]
  /**
   * Coded notes Settings localizes. Empty only for legacy runtime rows written
   * before code-ification, in which case `notes` is the whole truth.
   */
  noteDetails?: DerivedRuntimeNote[]
}

/**
 * Names one deterministic-runtime note with a stable code and typed params.
 *
 * Why this exists: the derived-state review card used to recover meaning from
 * backend English with regexes, so one punctuation edit in Rust silently dropped
 * zh copy back to English. `code` drives catalog lookup, `message` is the
 * fallback, and an empty `code` marks a deliberate diagnostic pass-through.
 */
export interface DerivedRuntimeNote {
  code: string
  message: string
  /** Browser profile the note is about, interpolated as `{profile}`. */
  profileId?: string | null
  /** Stable rebuild-mode id the shell turns into a localized label. */
  jobKind?: string | null
}

/**
 * Defines the typed shape for intelligence runtime snapshot.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface IntelligenceRuntimeSnapshot {
  queue: IntelligenceQueueStatus
  plugins: EnrichmentPluginStatus[]
  modules: DeterministicModuleRuntimeStatus[]
  recentJobs: IntelligenceJobOverview[]
  notes: string[]
}

/**
 * One page of a queue's jobs, newest first by when they were added. A job
 * keeps its place while it runs and finishes, so reading on with
 * `nextCursor` (the last job's id) neither skips nor repeats one.
 */
export interface JobPage<T> {
  items: T[]
  nextCursor: number | null
}

/** Request for `list_intelligence_jobs` and `list_ai_jobs`. */
export interface ListJobsRequest {
  cursor: number | null
  limit: number
}
/**
 * Represents a completed report that the UI can review after a run finishes.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface ClearDerivedIntelligenceReport {
  clearedVisitDerivedFactRows: number
  clearedDailyRollupRows: number
  clearedStructuralRows: number
  clearedRuntimeRows: number
  notes: string[]
}

/**
 * Defines the typed shape for ai provider secret input.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AiProviderSecretInput {
  providerId: string
  apiKey: string
}

/**
 * Describes a request payload in this front-end contract.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
/**
 * Which slice of the archive a re-embed run touches (W-AI-9 Sub-block D, 05 §7). Mirrors the Rust
 * `ReembedScope` (kebab-case on the wire). `incremental` is the historical default.
 */
export type ReembedScope = 'incremental' | 'working-set' | 'full'

export interface AiIndexRequest {
  providerId?: string | null
  fullRebuild: boolean
  clearOnly: boolean
  limit?: number | null
  /**
   * Which slice to re-embed (W-AI-9 Sub-block D). Optional + defaults to `incremental` on the backend
   * (`#[serde(default)]`), so omitting it preserves the historical "embed new/changed rows" behavior.
   */
  scope?: ReembedScope
}

/**
 * How much a re-embed run would index, shown before it starts (W-AI-9 Sub-block D, 05 §7). Mirrors
 * the Rust `ReembedEstimate`. No time: the rate depends on the engine and the machine.
 */
export interface ReembedEstimate {
  scope: ReembedScope
  pageCount: number
}

/**
 * Represents a completed report that the UI can review after a run finishes.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AiIndexReport {
  jobId?: number | null
  runId?: number | null
  providerId: string
  model: string
  indexedItems: number
  updatedItems: number
  skippedItems: number
  removedItems: number
  lastIndexedAt: string
  notes: string[]
}

/**
 * Describes a request payload in this front-end contract.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AiSearchRequest {
  query: string
  profileId?: string | null
  domain?: string | null
  limit?: number | null
  cursor?: string | null
  /**
   * The `is:starred` facet (W-AI-6): when true, restrict BOTH lexical and semantic recall to starred
   * pages. Optional/absent means unfiltered. Mirrors the Rust `starred_only` (`#[serde(default)]`).
   */
  starredOnly?: boolean | null
  /** Inclusive local days, `YYYY-MM-DD`. Like every filter, applied before ranking. */
  startDate?: string | null
  endDate?: string | null
  /** A browser (`chrome`, `firefox`, …), all of its profiles. */
  browserKind?: string | null
}

/**
 * Defines the typed shape for ai search result item.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AiSearchResultItem {
  historyId: number
  profileId: string
  url: string
  title?: string | null
  domain: string
  visitedAt: string
  score: number
  /**
   * Stable, locale-independent CODE for why the row matched — NEVER display copy.
   *
   * One of `lexical` / `semantic` / `lexical+semantic` / `lexical-date-ordered` / `recent-visit`,
   * optionally suffixed `+starred` when the starred boost promoted the row. The same field is quoted
   * verbatim into the model's context by the agent/`run_code`/MCP surfaces, so it cannot be prose;
   * the UI resolves it through `localizeAiMatchReason` (an unknown code renders verbatim).
   */
  matchReason: string
  /**
   * Capped excerpt of the matched page's enrichment summary (W-ENRICH-1, 06 §6; REACH-C3). The only
   * honest snippet on the Smart/semantic path: an enriched page's stored summary (≤180 chars,
   * CJK-safe), never a fabricated match-text chunk. Present only when the matched page has enrichment
   * text; absent/undefined for the (vast) majority of pages, where `matchReason` + the relevance band
   * carry the "why" and the row suppresses the affordance. Mirrors the Rust `enrichment_excerpt`
   * (`#[serde(default, skip_serializing_if = "Option::is_none")]`).
   */
  enrichmentExcerpt?: string | null
}

/**
 * Describes a response payload in this front-end contract.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
/**
 * Why the semantic index is stale. Stable, locale-independent reason CODE shared
 * by the AI-search degradation notes and the Settings index-health warning. Mirrors the Rust
 * `AiSemanticStaleness` (`#[serde(rename_all = "camelCase")]`). The FE NEVER renders these raw.
 */
export type AiSemanticStaleness = 'watermark' | 'enrichment'

/**
 * One stable, locale-independent AI-search degradation note CODE. Mirrors the Rust
 * `AiSearchNote` (`#[serde(tag = "code", rename_all = "camelCase")]`). The FE resolves each to
 * localized copy and NEVER renders these raw; the legacy English `AiSearchResponse.notes` are kept
 * only for the model-facing agent-tool path + the persisted run trace.
 */
export type AiSearchNote =
  | { code: 'lexicalFallbackNoProvider' }
  | { code: 'emptySemanticIndex' }
  | { code: 'semanticMatchesFilteredOut' }
  | { code: 'configDriftDimension' }
  | { code: 'configDriftFingerprint' }
  | { code: 'stale'; reason: AiSemanticStaleness }
  | { code: 'providerResolutionFailed'; reason: string }
  /** Searched the previous model's index; the chosen model's is not complete yet. */
  | { code: 'previousModel'; model: string }

export interface AiSearchResponse {
  total: number
  providerId: string
  model: string
  items: AiSearchResultItem[]
  /**
   * English degradation prose, retained for the model-facing agent-tool path + the persisted trace.
   * The user-facing surface reads `noteCodes` instead and never renders these raw.
   */
  notes: string[]
  /**
   * Stable degradation note CODES, additive (mirrors Rust `#[serde(default)]`). One
   * code per English note in `notes`, same order. The FE resolves each to localized copy.
   */
  noteCodes?: AiSearchNote[]
  nextCursor?: string | null
  /** Whether the search index covered the whole archive for this answer. */
  searchState?: SearchCoverage | null
}

/**
 * Conversational role for one streaming chat message (mirrors the backend `LlmRole`).
 */
export type AiChatRole = 'system' | 'user' | 'assistant' | 'tool'

/**
 * One message in a streaming chat request.
 */
export interface AiChatMessage {
  role: AiChatRole
  content: string
}

/**
 * Request payload for `ai_chat_send`. `providerId` defaults to the configured LLM provider when
 * omitted; `maxTokens` reserves local context space; it is not sent to the provider.
 *
 * W-AI-7 additive fields (all optional, mirroring the Rust `#[serde(default)]` so the frozen W-AI-1
 * plain-chat payload still serializes byte-for-byte):
 * - `toolsEnabled` switches the run from plain streaming chat to the tool-executing agent harness.
 *   Absent/false → plain chat (unchanged).
 * - `conversationId` / `messageId` link the durable agent run trace to the chat turn it answers
 *   (used only on the agent path; the backend FK self-heals if the conversation is not yet saved).
 */
export interface AiChatSendRequest {
  providerId?: string | null
  messages: AiChatMessage[]
  maxTokens?: number | null
  toolsEnabled?: boolean
  conversationId?: string | null
  messageId?: string | null
  /** The model to answer with; the service's default when absent. */
  model?: string | null
}

/** Request for `ai_chat_context`: the service and model Ask would answer with next. */
export interface AiChatContextRequest {
  providerId?: string | null
  model?: string | null
}

/**
 * How much a model can read at a time (`null` when its service does not
 * say) and how much every request already takes: the instructions, the
 * tools' descriptions and the room kept for the answer.
 */
export interface AiChatContext {
  model: string
  contextLength: number | null
  fixedTokens: number
}

/**
 * Acknowledgement from `ai_chat_send`: the run id used to subscribe to and cancel the stream.
 */
export interface AiChatSendAck {
  runId: string
}

/**
 * Result of `ai_chat_cancel`: whether a live run with the given id was found and asked to stop.
 */
export interface AiChatCancelResult {
  cancelled: boolean
}

/**
 * One cited history page surfaced by the agent run (W-AI-7), streamed in the terminal `citations`
 * chunk. Mirrors the Rust `AiCitation` (camelCase serde). `canonicalUrl` is the W-STAR star key,
 * resolved backend-side, so the UI can star a cited page directly without re-normalizing the url.
 */
export interface AiChatCitation {
  historyId: number
  profileId: string
  url: string
  title?: string | null
  visitedAt: string
  score?: number | null
  /** W-STAR star key (canonicalized URL). Present on the agent path; absent on legacy citations. */
  canonicalUrl?: string | null
}

/**
 * Which hard sandbox limit (if any) stopped a code-mode `run_code` script (W-AI-8 WU-5). The wire
 * values are the kebab tokens the Rust `LimitsHit` serializes (`#[serde(rename_all = "kebab-case")]`):
 * `time` (wall-time deadline), `memory` (memory cap), `host-calls` (host-call budget), `output`
 * (output byte cap), `cancelled` (user cancelled). The FE maps each to a localized chip; it is never
 * shown raw. Absent when the script finished within every bound.
 */
export type LimitsHit =
  | 'time'
  | 'memory'
  | 'host-calls'
  | 'output'
  | 'cancelled'

/**
 * A summary of one host-API call a code-mode script made (W-AI-8 WU-5), mirroring the Rust
 * `HostCallRecord` (camelCase serde). The STRUCTURED fields drive a translatable, per-row timeline:
 *
 * - `query` / `plane` / `limit` are populated for a `query_history` call (`plane` is the stable
 *   lowercase token `hybrid` / `vector` / `bm25`).
 * - `requestedIds` is populated for a `fetch_visits` call (how many visit ids it asked for).
 *
 * Each per-function field is absent when its call did not use it (`skip_serializing_if` on the Rust
 * side), so a record only advertises the args its function used. `argsSummary` is a NON-localized,
 * debug-only fallback (the same string a Rust log shows) — the FE composes its visible row from the
 * structured fields and never renders `argsSummary`. `rowCount` is how many rows the call returned.
 */
export interface HostCallRecord {
  function: string
  query?: string
  /** Stable lowercase plane token (`hybrid` / `vector` / `bm25`); present on `query_history`. */
  plane?: string
  limit?: number
  /** Count of visit ids requested; present on `fetch_visits`. */
  requestedIds?: number
  /** Non-localized debug fallback; the FE renders from the structured fields, never from this. */
  argsSummary: string
  rowCount: number
}

/**
 * One streamed chat chunk delivered over `pathkeep://ai-stream`.
 *
 * The `kind` tag routes each chunk into its UI lane: visible `token` text, model `reasoning`,
 * a requested `toolCall`, the terminal `done` marker, or a terminal `error` with a message.
 *
 * W-AI-7 is ADDITIVE (mirrors the Rust `AiChatStreamChunk`): `toolCall` gains an optional `callId`
 * (correlation id, present on the agent path); `toolResult` carries the executed result correlated
 * by `callId`; `usage` reports per-turn token accounting; `citations` carries the run's accumulated
 * evidence rows once, right before `done`. No existing variant changed shape, so the plain
 * (tools-off) stream is byte-for-byte unaffected.
 *
 * W-AI-8 WU-5 is also ADDITIVE on `toolResult`: a code-mode (`run_code`) result appends `codeSource`
 * (the verbatim script that ran), `hostCalls` (its host-call timeline), and `limitsHit` (the hard
 * sandbox limit, if any). These are absent for the search tools, so the W-AI-7 search step stays
 * byte-for-byte unchanged.
 */
export type AiChatStreamChunk =
  | { kind: 'token'; text: string }
  | { kind: 'reasoning'; text: string }
  | { kind: 'toolCall'; name: string; arguments: string; callId?: string }
  | {
      kind: 'toolResult'
      callId: string
      name: string
      result: string
      isError: boolean
      /** The verbatim `run_code` script (transparency); absent for the search tools. */
      codeSource?: string
      /** The `run_code` host-call timeline; absent/empty for the search tools. */
      hostCalls?: HostCallRecord[]
      /** Which hard sandbox limit bounded a `run_code` script, if any. */
      limitsHit?: LimitsHit
    }
  | { kind: 'usage'; promptTokens: number; completionTokens: number }
  | { kind: 'citations'; citations: AiChatCitation[] }
  | { kind: 'note'; code: AiAgentNote }
  | { kind: 'done' }
  | { kind: 'error'; message: string }

/**
 * One stable, locale-independent USER-facing agent-harness control note CODE.
 * Mirrors the Rust `AiAgentNote` (`#[serde(tag = "code", rename_all = "camelCase")]`), streamed inside
 * an `AiChatStreamChunk` `note` chunk. The FE resolves each to localized copy; these are the
 * user-facing twin of the harness's model-facing English (which stays English to steer the LLM). The
 * FE NEVER renders these raw.
 */
export type AiAgentNote =
  | { code: 'maxStepsReached' }
  | { code: 'tokenBudgetReached' }
  | { code: 'toolCallingUnavailable' }
  | { code: 'searchIndexIncomplete' }
  | { code: 'contextTooSmall' }
  | { code: 'sourcesNotSaved' }

/**
 * Envelope emitted on `pathkeep://ai-stream` pairing a chunk with its run id.
 */
export interface AiChatStreamEvent {
  runId: string
  chunk: AiChatStreamChunk
}

/**
 * One pinned evidence row reconstructed for a reopened assistant turn (W-AI-7 WU-7). Mirrors the
 * Rust `AgentCitation` (camelCase serde). Sourced from the durable `agent_citations` journal so a
 * reopened conversation renders the same starrable evidence the live turn streamed; `profileId` is
 * empty on a reconstructed row (the journal pins the star key, not the profile).
 */
export interface AgentCitation {
  historyId: number
  /** Empty on a reconstructed citation: the journal does not retain the profile. */
  profileId: string
  url: string
  title?: string | null
  /** ISO visit time; empty string when the journaled row had no timestamp. */
  visitedAt: string
  score?: number | null
  /** W-STAR star key (canonicalized url). */
  canonicalUrl?: string | null
}

/**
 * Per-turn token accounting reconstructed for a reopened assistant turn (W-AI-7 WU-7). Mirrors the
 * Rust `AgentUsage`; sourced from the `agent_runs` header that answered the message.
 */
export interface AgentUsage {
  promptTokens: number
  completionTokens: number
}

/**
 * One persisted assistant-chat message (mirrors the backend `AgentMessage` and the in-memory
 * `ChatMessage` shape). `toolCallsJson` is the serialized `AssistantToolCall[]` exactly as it was
 * rendered; persisting it as opaque JSON keeps the store decoupled from the evolving tool schema.
 *
 * W-AI-7 WU-7: `citations` and `usage` are load-only RECONSTRUCTION fields. They are not stored on
 * the message row — `loadAiConversation` joins the message's agent run → its pinned citations + token
 * tally — so a reopened conversation shows the same evidence rows + star keys + token footer the live
 * turn did. They are absent on the save payload (the backend skips them), so persist-on-finalize is
 * unchanged.
 */
export interface AgentMessage {
  id: string
  /** `'user'` or `'assistant'`. */
  role: string
  content: string
  reasoning?: string | null
  toolCallsJson?: string | null
  /** Terminal turn status (`done` / `error` / `cancelled`); absent for user messages. */
  status?: string | null
  /** Reconstructed evidence rows for this assistant turn (load-only; omitted on save). */
  citations?: AgentCitation[] | null
  /** Reconstructed per-turn token usage (load-only; omitted on save). */
  usage?: AgentUsage | null
  /** The service and model that wrote this answer (load-only; recorded by the run). */
  providerId?: string | null
  model?: string | null
}

/**
 * Lightweight conversation row for the chat-history explorer list (no message bodies).
 */
export interface AgentConversationSummary {
  id: string
  title: string
  /** LLM provider id active when saved (display only; never a model id). */
  providerId?: string | null
  createdAt: string
  updatedAt: string
  messageCount: number
}

/**
 * One conversation plus its full (bounded) message transcript. The summary fields are flattened
 * onto this object by the backend (`#[serde(flatten)]`), so it carries both the list metadata and
 * the messages.
 */
export interface AgentConversationDetail extends AgentConversationSummary {
  messages: AgentMessage[]
}

/**
 * Request payload for `save_ai_conversation`: upsert a conversation + replace its messages. When
 * `title` is omitted/blank the backend derives one from the first user message.
 */
export interface SaveAgentConversationRequest {
  id: string
  title?: string | null
  providerId?: string | null
  messages: AgentMessage[]
}

/**
 * Request payload for `list_ai_conversations`: a bounded, newest-first page cap.
 */
export interface ListAgentConversationsRequest {
  /** Page size; the cursor reaches every conversation. */
  limit?: number | null
  cursor?: string | null
}

/**
 * Response for `list_ai_conversations`.
 */
export interface AgentConversationListResponse {
  conversations: AgentConversationSummary[]
  nextCursor: string | null
}

/**
 * Request payload for `rename_ai_conversation`.
 */
export interface RenameAgentConversationRequest {
  id: string
  title: string
}

/** Request for `export_ai_conversation`: the conversation and the file's words, in the interface's language. */
export interface ExportAgentConversationRequest {
  id: string
  labels: {
    untitled: string
    you: string
    assistant: string
    pagesUsed: string
  }
}

/**
 * What `export_ai_conversation` did: saved where the person chose in the
 * system's save dialog, nothing because they cancelled it, or nothing because
 * the conversation is no longer saved.
 */
export type ConversationExportOutcome =
  | { status: 'saved'; path: string; bytes: number }
  | { status: 'cancelled' }
  | { status: 'gone' }

/**
 * Result of `delete_ai_conversation`: whether a row with the id existed and was removed.
 */
export interface DeleteAgentConversationResult {
  deleted: boolean
}

/**
 * Represents the preview payload shown before a write or high-risk action happens.
 *
 * These type contracts are read directly by routes, helper modules, and preview fixtures, so a reader should be able to understand the shape without hunting through call sites.
 */
export interface AiIntegrationPreview {
  mcpCommand: string
  consentSummary: string
  manualSteps: string[]
  capabilityNotes: string[]
  scopeBoundary: string[]
  auditTrace: string[]
  generatedFiles: ScheduleGeneratedFile[]
  warnings: string[]
}

/**
 * User-facing on/off preference for one site-content-fetch extractor (W-ENRICH-1, 06 §2a).
 *
 * Mirrors `vault_core::models::intelligence::ContentFetchExtractorPreference` (camelCase serde).
 * `extractorId` is one of the built-in extractor ids ("github-repo", "generic-readable", …);
 * `enabled` only takes effect WHEN the master `ContentFetchSettings.enabled` switch is on.
 */
export interface ContentFetchExtractorPreference {
  extractorId: string
  enabled: boolean
}

/**
 * Per-domain allow/block rule for site content fetching (W-ENRICH-1, 06 §2a).
 *
 * Mirrors `vault_core::models::intelligence::ContentFetchDomainRule` (camelCase serde).
 * `allowed = false` blocks the domain even when the master switch is on; `allowed = true` is an
 * explicit allow (reserved for a future allow-list-only mode). The MVP runner only treats
 * `allowed = false` as load-bearing, since the master switch is the gate.
 */
export interface ContentFetchDomainRule {
  domain: string
  allowed: boolean
}

/**
 * Settings-facing content-fetch consent + live status surface (W-ENRICH-1, 06 §6).
 *
 * Mirrors `vault_core::models::intelligence::ContentFetchSettings` (camelCase serde). `enabled` is
 * the hard-default-OFF master consent switch — when false the whole content-fetch plane is inert and
 * NO network egress happens. The `*Jobs` / `storedRecords` counts are a small live status so the
 * consent panel can show fetch progress without a separate query.
 */
export interface ContentFetchSettings {
  enabled: boolean
  extractors: ContentFetchExtractorPreference[]
  domains: ContentFetchDomainRule[]
  queuedJobs: number
  runningJobs: number
  failedJobs: number
  storedRecords: number
}

/**
 * One stored site-content enrichment row for a visit/URL, for the detail panel (W-ENRICH-1, 06 §6).
 *
 * Mirrors `vault_core::models::intelligence::VisitEnrichmentRecord` (camelCase serde). Only the
 * capped `summary` + structured `metadataJson` ship here — the full body stays in the
 * content-addressed blob and is never sent to the FE. `fetchStatus` is honest (success | empty |
 * blocked | fetch-error | …) so the panel can render a real failure state instead of pretending.
 * `metadataJson` is an opaque JSON string the FE parses for chips (GitHub topics/desc, …); it stays
 * a string so this model is decoupled from per-extractor schema.
 */
export interface VisitEnrichmentRecord {
  contentSource: string
  fetchStatus: string
  fetchedAt: string
  readableTitle?: string | null
  summary?: string | null
  extractorVersion?: number | null
  metadataJson?: string | null
  finalUrl?: string | null
  httpStatus?: number | null
  refetchAfter?: string | null
}

/**
 * Request payload for the manual "fetch now" PME trigger (W-ENRICH-1, 06 §6).
 *
 * Mirrors `vault_core::models::intelligence::ContentFetchNowRequest` (camelCase serde).
 */
export interface ContentFetchNowRequest {
  historyId: number
  profileId: string
  url: string
  title?: string | null
}

/**
 * Result of a manual "fetch now" enqueue (W-ENRICH-1, 06 §6).
 *
 * Mirrors `vault_core::models::intelligence::ContentFetchNowResult` (camelCase serde). `state` is
 * `queued` / `running` / `disabled` — `disabled` is returned (without queuing) when consent is off
 * for this URL, which the FE maps to an honest "consent required" affordance. `note` is a
 * localisation-key-friendly hint the FE maps to copy (never raw prose committed to the contract).
 */
export interface ContentFetchNowResult {
  jobId: number
  state: string
  note: string
}

/**
 * One request an outside app made through PathKeep's MCP server
 * (`load_mcp_queries`): which tool, what it asked and how much came back.
 */
export interface McpQueryRecord {
  id: number
  at: string
  /** `search-history`, `archive-status` or `usage-guide`. */
  tool: string
  query?: string | null
  domain?: string | null
  profileId?: string | null
  /** How many pages matched a search, across the archive. */
  results?: number | null
  /**
   * How many of them the app was given (one page of results); absent on
   * requests recorded before PathKeep kept it.
   */
  returned?: number | null
  notes: string[]
}
