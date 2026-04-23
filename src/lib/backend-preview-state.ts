/**
 * @file backend-preview-state.ts
 * @description Mutable browser-preview state owner for the preview backend facade and test harness.
 * @module lib/backend-preview-state
 *
 * ## Responsibilities
 * - Define the mutable in-memory state shape behind browser-preview commands.
 * - Normalize preview config, app-lock state, AI queue state, and deterministic runtime state after mutations.
 * - Create one canonical preview state baseline so browser-preview reads and tests stay aligned.
 *
 * ## Not responsible for
 * - Dispatching preview commands or deciding route-level UX behavior.
 * - Owning static fixture payloads such as the seeded snapshot/history/runtime rows.
 * - Replacing the typed desktop transport or `backend-client/*`.
 *
 * ## Dependencies
 * - Depends on typed contracts from `./types`, shared enrichment helpers, and static preview fixtures.
 * - Reuses the same deterministic defaults consumed by browser-preview and test harnesses.
 *
 * ## Performance notes
 * - State sync helpers are hot on every preview mutation, so they should stay cheap, deterministic, and free of unbounded scans beyond the in-memory fixture surface.
 */

import {
  READABLE_CONTENT_REFETCH_PLUGIN_ID,
  resolveEnrichmentSettings,
} from './enrichment'
import { normalizeExplorerBackgroundPrefetchPages } from './explorer-preferences'
import {
  mockHistory,
  mockIntelligenceRuntime,
  mockSnapshot,
} from './backend-preview-fixtures'
import type {
  AiQueueJob,
  AiQueueStatus,
  AppConfig,
  AppLockStatus,
  AppSnapshot,
  ImportBatchDetail,
  S3CredentialInput,
  SchedulePlan,
  ScheduleStatus,
} from './types'
import type { SearchEngineRule } from './core-intelligence'

/**
 * Holds the mutable browser-preview backend state that command handlers read and mutate.
 *
 * The shape is intentionally broader than any single command because preview mode needs one
 * shared truth for shell state, review fixtures, and test-driven mutations.
 */
export interface MockBackendState {
  snapshot: AppSnapshot
  history: typeof mockHistory
  keyringSecret: string | null
  s3Credentials: S3CredentialInput | null
  appLockPasscode: string | null
  appLockRecoveryHint: string | null
  biometricState: AppLockStatus['biometricState']
  importBatchDetails: Record<number, ImportBatchDetail>
  schedulePlanOverrides: Partial<
    Record<'macos' | 'windows' | 'linux', SchedulePlan>
  >
  scheduleStatusOverrides: Partial<
    Record<'macos' | 'windows' | 'linux', ScheduleStatus>
  >
  intelligenceRuntime: typeof mockIntelligenceRuntime
  queueJobs: AiQueueJob[]
  nextAiJobId: number
  nextImportBatchId: number
  lastRemoteBundlePath: string | null
  derivedStateCleared: boolean
  searchEngineRules: SearchEngineRule[]
}

function buildMockSearchEngineRules(): SearchEngineRule[] {
  return [
    {
      ruleId: 'builtin:google',
      engineId: 'google',
      displayName: 'Google',
      hostPattern: 'google.com',
      pathPrefix: '/search',
      queryParamKey: 'q',
      enabled: true,
      note: null,
      exampleUrl: 'https://www.google.com/search?q=sqlite+wal',
      builtIn: true,
    },
    {
      ruleId: 'builtin:bilibili',
      engineId: 'bilibili',
      displayName: 'BiliBili',
      hostPattern: 'search.bilibili.com',
      pathPrefix: '/all',
      queryParamKey: 'keyword',
      enabled: true,
      note: null,
      exampleUrl: 'https://search.bilibili.com/all?keyword=sqlite+wal',
      builtIn: true,
    },
    {
      ruleId: 'builtin:github',
      engineId: 'github',
      displayName: 'GitHub',
      hostPattern: 'github.com',
      pathPrefix: '/search',
      queryParamKey: 'q',
      enabled: true,
      note: null,
      exampleUrl: 'https://github.com/search?q=sqlite+wal',
      builtIn: true,
    },
    {
      ruleId: 'custom:docs-search',
      engineId: 'docs-search',
      displayName: 'Docs Search',
      hostPattern: 'docs.example.com',
      pathPrefix: '/search',
      queryParamKey: 'query',
      enabled: true,
      note: 'Preview custom rule',
      exampleUrl: 'https://docs.example.com/search?query=sqlite+wal',
      builtIn: false,
    },
  ]
}

/**
 * Coerces preview config back into the same normalized shape that shipped frontend code expects.
 *
 * This protects browser-preview from drifting into impossible config states after tests or
 * command handlers mutate the in-memory fixture.
 */
export function normalizeMockConfig(
  config: AppConfig,
  s3Credentials: S3CredentialInput | null = null,
): AppConfig {
  return {
    ...config,
    explorerBackgroundPrefetchPages: normalizeExplorerBackgroundPrefetchPages(
      config.explorerBackgroundPrefetchPages,
    ),
    appLock: {
      ...config.appLock,
      idleTimeoutMinutes: Math.min(
        60,
        Math.max(1, config.appLock.idleTimeoutMinutes),
      ),
    },
    enrichment: resolveEnrichmentSettings(config.enrichment),
    remoteBackup: {
      ...config.remoteBackup,
      credentialsSaved: Boolean(s3Credentials),
    },
  }
}

/**
 * Rebuilds derived app-lock state after preview config or credential mutations.
 *
 * The shell and settings surfaces both rely on these derived warnings and degradation notes,
 * so preview mode has to keep them synchronized instead of leaving stale lock metadata around.
 */
export function syncMockAppLockState(state: MockBackendState) {
  const passcodeConfigured = Boolean(state.appLockPasscode)
  const enabled = state.snapshot.config.appLock.enabled
  const passcodeEnabled = state.snapshot.config.appLock.passcodeEnabled
  const biometricEnabled = state.snapshot.config.appLock.biometricEnabled
  const pendingPasscodeWarning =
    enabled && passcodeEnabled && !passcodeConfigured
  const locked = enabled ? state.snapshot.appLockStatus.locked : false
  const lockReason = locked ? state.snapshot.appLockStatus.lockReason : null
  const lockedAt = locked ? state.snapshot.appLockStatus.lockedAt : null
  const lastUnlockedAt = state.snapshot.appLockStatus.lastUnlockedAt

  state.snapshot.config.appLock = {
    ...state.snapshot.config.appLock,
    passcodeConfigured,
    recoveryHint: state.appLockRecoveryHint,
  }
  state.snapshot.appLockStatus = {
    ...state.snapshot.appLockStatus,
    enabled,
    locked,
    idleTimeoutMinutes: state.snapshot.config.appLock.idleTimeoutMinutes,
    biometricAvailable: state.biometricState === 'touch-id-available',
    biometricEnabled,
    biometricState: state.biometricState,
    passcodeEnabled,
    passcodeConfigured,
    recoveryHint: state.appLockRecoveryHint,
    lockReason,
    lockedAt,
    lastUnlockedAt,
    warnings: pendingPasscodeWarning
      ? ['Set an app lock passcode before relying on session lock.']
      : biometricEnabled && state.biometricState !== 'touch-id-available'
        ? [
            state.biometricState === 'touch-id-unavailable'
              ? 'Touch ID is unavailable on this Mac right now, so PathKeep falls back to the app-lock passcode.'
              : 'Biometric unlock is reserved for future platform integration; this preview falls back to the app-lock passcode.',
          ]
        : [],
    degradationNotes: [
      'App Lock only protects the PathKeep UI session. Archive encryption still protects data at rest.',
      state.biometricState === 'touch-id-available'
        ? 'Touch ID is available on this Mac and can unlock the current PathKeep session.'
        : state.biometricState === 'touch-id-unavailable'
          ? 'Touch ID is unavailable on this Mac right now, so PathKeep falls back to the app-lock passcode.'
          : 'Biometric unlock is reserved for future platform integration; this preview falls back to the app-lock passcode.',
    ],
  }
}

/**
 * Rejects preview commands that should not read archive data while App Lock is active.
 *
 * This keeps browser-preview honest: lock state should block the same shell reads that the
 * real desktop runtime would refuse.
 */
export function ensureMockUnlocked(command: string, state: MockBackendState) {
  if (!state.snapshot.appLockStatus.locked) {
    return
  }

  if (
    command === 'app_build_info' ||
    command === 'app_lock_status' ||
    command === 'unlock_app_session' ||
    command === 'open_path_in_file_manager' ||
    command === 'open_external_url'
  ) {
    return
  }

  throw new Error(
    'PathKeep is currently locked. Unlock the app before requesting archive data.',
  )
}

/**
 * Guards preview config writes from entering app-lock states the real product would reject.
 *
 * This catches invalid enablement flows early so route tests see truthful failures instead of
 * silently mutating the fixture into impossible combinations.
 */
export function validateMockAppLockConfig(
  state: MockBackendState,
  config: AppConfig,
) {
  if (!config.appLock.enabled) {
    return
  }

  if (
    config.appLock.biometricEnabled &&
    state.biometricState !== 'touch-id-available'
  ) {
    throw new Error(
      state.biometricState === 'touch-id-unavailable'
        ? 'Touch ID is unavailable on this Mac right now. Use the app lock passcode instead.'
        : 'Biometric unlock is not available in the current desktop build.',
    )
  }

  if (!config.appLock.passcodeEnabled) {
    throw new Error(
      'Enable a passcode before turning on App Lock in this build.',
    )
  }

  if (!state.appLockPasscode) {
    throw new Error('Set an app lock passcode before turning on App Lock.')
  }
}

/**
 * Summarizes the preview AI queue into the same compact shape the UI consumes from desktop.
 */
export function buildMockQueueStatus(state: MockBackendState): AiQueueStatus {
  return {
    paused: state.snapshot.config.ai.jobQueuePaused,
    concurrency: state.snapshot.config.ai.jobQueueConcurrency,
    queued: state.queueJobs.filter(
      (job) => job.state === 'queued' || job.state === 'paused',
    ).length,
    running: state.queueJobs.filter((job) => job.state === 'running').length,
    failed: state.queueJobs.filter((job) => job.state === 'failed').length,
    recentJobs: state.queueJobs.slice(0, 8).map((job) => structuredClone(job)),
  }
}

/**
 * Copies queue totals and recent jobs back into `snapshot.aiStatus` after preview queue mutations.
 */
export function syncMockAiStatus(state: MockBackendState) {
  const queue = buildMockQueueStatus(state)
  state.snapshot.aiStatus.queuePaused = queue.paused
  state.snapshot.aiStatus.queueConcurrency = queue.concurrency
  state.snapshot.aiStatus.queuedJobs = queue.queued
  state.snapshot.aiStatus.runningJobs = queue.running
  state.snapshot.aiStatus.failedJobs = queue.failed
  state.snapshot.aiStatus.recentJobs = structuredClone(queue.recentJobs)
}

/**
 * Rebuilds the deterministic runtime digest after enrichment toggles or recent-job mutations.
 *
 * The Jobs and Intelligence surfaces both read this object directly, so preview mode needs
 * one shared recomputation path instead of route-local patching.
 */
export function syncMockIntelligenceRuntime(state: MockBackendState) {
  const enabledById = new Map(
    resolveEnrichmentSettings(state.snapshot.config.enrichment).plugins.map(
      (plugin) => [plugin.id, plugin.enabled],
    ),
  )
  const moduleEnabledById = new Map(
    state.snapshot.config.deterministic.modules.map((module) => [
      module.id,
      module.enabled,
    ]),
  )
  const recentJobs = state.intelligenceRuntime.recentJobs
  const activityTimes = recentJobs
    .flatMap((job) => [job.finishedAt, job.startedAt, job.createdAt])
    .filter((value): value is string => Boolean(value))
  const lastActivityAt =
    activityTimes.length > 0
      ? activityTimes.sort()[activityTimes.length - 1]
      : null

  state.intelligenceRuntime.queue = {
    queued: recentJobs.filter((job) => job.state === 'queued').length,
    running: recentJobs.filter((job) => job.state === 'running').length,
    succeeded: recentJobs.filter((job) => job.state === 'succeeded').length,
    failed: recentJobs.filter((job) => job.state === 'failed').length,
    cancelled: recentJobs.filter((job) => job.state === 'cancelled').length,
    lastActivityAt,
  }
  state.intelligenceRuntime.plugins = state.intelligenceRuntime.plugins.map(
    (plugin) => ({
      ...plugin,
      enabled: enabledById.get(plugin.pluginId) ?? plugin.enabled,
      queuedJobs: recentJobs.filter(
        (job) => job.pluginId === plugin.pluginId && job.state === 'queued',
      ).length,
      runningJobs: recentJobs.filter(
        (job) => job.pluginId === plugin.pluginId && job.state === 'running',
      ).length,
      failedJobs: recentJobs.filter(
        (job) => job.pluginId === plugin.pluginId && job.state === 'failed',
      ).length,
      lastCompletedAt:
        recentJobs
          .filter(
            (job) =>
              job.pluginId === plugin.pluginId &&
              job.state === 'succeeded' &&
              job.finishedAt,
          )
          .map((job) => job.finishedAt!)
          .sort()
          .at(-1) ?? null,
      lastError:
        recentJobs.find(
          (job) => job.pluginId === plugin.pluginId && job.state === 'failed',
        )?.lastError ?? null,
    }),
  )
  state.intelligenceRuntime.modules = state.intelligenceRuntime.modules.map(
    (module) => ({
      ...module,
      enabled: moduleEnabledById.get(module.moduleId) ?? module.enabled,
      status:
        moduleEnabledById.get(module.moduleId) === false
          ? 'disabled'
          : module.status,
      notes:
        moduleEnabledById.get(module.moduleId) === false
          ? ['Disabled in Settings.']
          : module.notes,
    }),
  )
  state.intelligenceRuntime.notes = [
    'Browser preview mode shows a deterministic queue/runtime fixture.',
    enabledById.get(READABLE_CONTENT_REFETCH_PLUGIN_ID) === false
      ? 'Readable content refetch is disabled, so queued network enrichment will stay paused until you re-enable it.'
      : 'Built-in enrichment stays inside the first-party runtime boundary in browser preview mode.',
  ]
}

/**
 * Creates the full mutable preview state baseline consumed by browser-preview commands and tests.
 *
 * This should stay the only entry point for resetting preview state so every caller gets the
 * same normalized config, lock metadata, and deterministic runtime digest.
 */
export function createMockState(): MockBackendState {
  const state: MockBackendState = {
    snapshot: structuredClone(mockSnapshot),
    history: structuredClone(mockHistory),
    keyringSecret: null,
    s3Credentials: null,
    appLockPasscode: null,
    appLockRecoveryHint: null,
    biometricState: 'unsupported',
    importBatchDetails: {},
    schedulePlanOverrides: {},
    scheduleStatusOverrides: {},
    intelligenceRuntime: structuredClone(mockIntelligenceRuntime),
    queueJobs: [
      {
        id: 2,
        jobType: 'index-build',
        state: 'failed',
        priority: 70,
        attempt: 1,
        maxAttempts: 3,
        runId: null,
        summary: 'Preview queue fixture needs a replay.',
        queuedAt: new Date(Date.now() - 120_000).toISOString(),
        availableAt: new Date(Date.now() - 60_000).toISOString(),
        startedAt: new Date(Date.now() - 110_000).toISOString(),
        finishedAt: new Date(Date.now() - 100_000).toISOString(),
        heartbeatAt: new Date(Date.now() - 105_000).toISOString(),
        errorCode: 'network-error',
        errorMessage: 'Preview transport timed out.',
      },
      {
        id: 1,
        jobType: 'assistant',
        state: 'queued',
        priority: 100,
        attempt: 0,
        maxAttempts: 1,
        runId: null,
        summary: 'What did I read about LanceDB?',
        queuedAt: new Date(Date.now() - 30_000).toISOString(),
        availableAt: new Date(Date.now() - 30_000).toISOString(),
        startedAt: null,
        finishedAt: null,
        heartbeatAt: null,
        errorCode: null,
        errorMessage: null,
      },
    ],
    nextAiJobId: 3,
    nextImportBatchId: 1,
    lastRemoteBundlePath: null,
    derivedStateCleared: false,
    searchEngineRules: buildMockSearchEngineRules(),
  }
  state.snapshot.config = normalizeMockConfig(
    state.snapshot.config,
    state.s3Credentials,
  )
  syncMockAppLockState(state)
  syncMockAiStatus(state)
  syncMockIntelligenceRuntime(state)
  return state
}
