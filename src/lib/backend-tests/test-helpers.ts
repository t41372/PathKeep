/**
 * @file test-helpers.ts
 * @description Shared backend preview test fixtures used by the split backend facade suites.
 * @module lib/backend-tests/test-helpers
 *
 * ## Responsibilities
 * - Expose one canonical preview `AppConfig` fixture for backend facade tests.
 * - Expose one canonical manual-review `SchedulePlan` fixture for schedule passthrough tests.
 * - Keep split suites aligned on the same baseline assumptions without duplicating giant literal objects.
 *
 * ## Not responsible for
 * - Mocking Tauri transport; each suite still owns its local `vi.hoisted` setup.
 * - Resetting backend preview state; each suite still decides when to call `backendTestHarness.reset()`.
 * - Hiding which fields matter to a given test case; suites should still override values explicitly when behavior depends on them.
 *
 * ## Dependencies
 * - Depends only on typed contracts from `../types`.
 *
 * ## Performance notes
 * - These fixtures are tiny immutable literals reused by tests, so sharing them avoids repeated giant object declarations without affecting runtime behavior.
 */

import type { AppConfig, SchedulePlan } from '../types'

/**
 * Provides the baseline preview config shared by backend facade tests before each suite mutates specific fields.
 *
 * Keeping one canonical config here makes it obvious when a test is intentionally changing archive, AI, or app-lock behavior
 * instead of silently drifting because it copied an outdated literal from another suite.
 */
export const previewConfigFixture: AppConfig = {
  initialized: false,
  archiveMode: 'Encrypted',
  preferredLanguage: 'system',
  dueAfterHours: 72,
  scheduleCheckIntervalHours: 6,
  checkpointDays: 90,
  captureFavicons: true,
  selectedProfileIds: ['chrome:Default'],
  gitEnabled: true,
  rememberDatabaseKeyInKeyring: false,
  appAutostart: false,
  appLock: {
    enabled: false,
    idleTimeoutMinutes: 5,
    biometricEnabled: false,
    passcodeEnabled: true,
    passcodeConfigured: false,
    recoveryHint: null,
  },
  analytics: {
    enabled: false,
    consentGrantedAt: null,
  },
  remoteBackup: {
    enabled: false,
    bucket: '',
    region: 'us-east-1',
    endpoint: null,
    prefix: 'pathkeep',
    pathStyle: true,
    uploadAfterBackup: false,
    credentialsSaved: false,
    lastUploadedAt: null,
    lastUploadedObjectKey: null,
    lastError: null,
  },
  enrichment: {
    plugins: [
      {
        id: 'readable-content-refetch',
        enabled: true,
        version: 'm4-v1',
      },
    ],
  },
  deterministic: {
    modules: [
      { id: 'visit-derived-facts', enabled: true, version: 'ci-v1' },
      { id: 'daily-rollups', enabled: true, version: 'ci-v1' },
      { id: 'sessions', enabled: true, version: 'ci-v1' },
      { id: 'search-trails', enabled: true, version: 'ci-v1' },
      { id: 'refind-pages', enabled: true, version: 'ci-v1' },
      { id: 'activity-mix', enabled: true, version: 'ci-v1' },
      { id: 'search-effectiveness', enabled: true, version: 'ci-v1' },
      { id: 'domain-deep-dive', enabled: true, version: 'ci-v1' },
    ],
  },
  ai: {
    enabled: false,
    assistantEnabled: false,
    semanticIndexEnabled: false,
    mcpEnabled: false,
    skillEnabled: false,
    autoIndexAfterBackup: false,
    jobQueuePaused: false,
    jobQueueConcurrency: 1,
    enrichmentEnabled: true,
    enrichmentPlugins: [
      { pluginId: 'title-normalization', enabled: true },
      { pluginId: 'readable-content-refetch', enabled: true },
    ],
    llmProviderId: null,
    embeddingProviderId: null,
    retrievalTopK: 8,
    assistantSystemPrompt:
      'You are an audit-first history research assistant. Use the available browser history evidence before answering. Be explicit about uncertainty and cite the history rows you relied on.',
    llmProviders: [],
    embeddingProviders: [],
  },
}

/**
 * Provides the manual-review schedule plan fixture shared by preview and Tauri passthrough tests.
 *
 * The tests only care that the facade forwards this plan unchanged, so centralizing it keeps those assertions
 * focused on command routing instead of maintaining duplicate placeholder plans.
 */
export const schedulePlanFixture: SchedulePlan = {
  platform: 'macos',
  label: 'dev.example.pathkeep.backup',
  executablePath: '/Applications/PathKeep.app',
  generatedFiles: [],
  manualSteps: [],
  applyCommands: [],
  rollbackCommands: [],
  applySupported: false,
}
