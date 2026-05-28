/**
 * @file test-helpers.tsx
 * @description Shared shell-test harness for the split `src/app/index.test.tsx` suites.
 * @module app/index-tests
 *
 * ## Responsibilities
 * - Hold the one canonical shell-test seed contract for onboarding, lock, explorer, settings, and router suites.
 * - Centralize the reset behavior, translators, DOM narrowing helper, and schedule fixture used across split suites.
 * - Keep archive bootstrap helpers in one place so new test files do not clone config fixtures.
 *
 * ## Not responsible for
 * - Owning route-specific assertions or deciding which suite covers which surface.
 * - Rendering the app itself; each suite still chooses the router and route under test.
 * - Introducing new mock layers beyond the concrete helpers already exercised by the original mega-suite.
 *
 * ## Dependencies
 * - Depends on the backend test harness, typed backend client, and shipped i18n helpers.
 * - Uses the same app config contract exercised by the original mega-suite.
 *
 * ## Performance notes
 * - Reuses seeded archive state and shared translators so suite splitting does not multiply shell bootstrap work unnecessarily.
 */

import { expect, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { backendTestHarness } from '../../lib/backend'
import { defaultExplorerBackgroundPrefetchPages } from '../../lib/explorer-preferences'
import { createNamespaceTranslator, createTranslator } from '../../lib/i18n'
import type { AppConfig } from '../../lib/types'

export const commonT = createTranslator('en')
export const dashboardT = createNamespaceTranslator('en', 'dashboard')
export const shellT = createNamespaceTranslator('en', 'shell')
export const onboardingT = createNamespaceTranslator('en', 'onboarding')
export const assistantT = createNamespaceTranslator('en', 'assistant')
export const intelligenceT = createNamespaceTranslator('en', 'intelligence')
export const scheduleT = createNamespaceTranslator('en', 'schedule')
export const securityT = createNamespaceTranslator('en', 'security')
export const settingsT = createNamespaceTranslator('en', 'settings')

const initializedConfig: AppConfig = {
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
  explorerBackgroundPrefetchPages: defaultExplorerBackgroundPrefetchPages,
  appLock: {
    enabled: false,
    idleTimeoutMinutes: 5,
    biometricEnabled: false,
    passcodeEnabled: true,
    passcodeConfigured: false,
    recoveryHint: null,
  },
  enrichment: {
    plugins: [
      {
        id: 'readable-content-refetch',
        enabled: true,
        version: 'diagnostic',
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
 * Restores mocks and resets the backend fixture to the same baseline every split suite
 * previously got from the mega-test's `beforeEach`.
 *
 * This keeps route-focused files independent without letting one suite leak seeded state
 * into the next run.
 */
export function resetAppShellHarness() {
  vi.restoreAllMocks()
  backendTestHarness.reset()
}

/**
 * Initializes the shell fixture archive without running a backup yet.
 *
 * Use this when a test needs the archive created but wants to mutate raw history rows
 * before read models are refreshed by a backup pass.
 */
export async function initializeArchiveOnly() {
  await backend.initializeArchive(initializedConfig, 'vault-passphrase')
}

/**
 * Seeds an initialized archive plus the first backup so shell routes can render against
 * realistic read models.
 *
 * This is the default path for route tests that care about the post-backup shell state
 * rather than the pre-backup bootstrap boundary.
 */
export async function seedArchiveRun() {
  await initializeArchiveOnly()
  await backend.runBackupNow(false)
}

/**
 * Enables one local LLM provider so settings and assistant suites can stay focused on
 * UI review behavior instead of rebuilding provider config boilerplate inline.
 */
export async function seedAiProviders() {
  const snapshot = await backend.getAppSnapshot()
  await backend.saveConfig({
    ...snapshot.config,
    ai: {
      ...snapshot.config.ai,
      enabled: true,
      llmProviderId: 'llm-local',
      llmProviders: [
        {
          id: 'llm-local',
          name: 'Local LLM',
          purpose: 'llm',
          requestFormat: 'openai',
          enabled: true,
          baseUrl: 'http://localhost:11434',
          apiKeySaved: false,
          defaultModel: 'qwen3:8b',
          modelCatalog: [],
          temperature: 0.2,
          maxTokens: 1200,
          dimensions: null,
          notes: null,
        },
      ],
      embeddingProviders: [],
    },
  })
}

/**
 * Narrows a nullable DOM lookup to `HTMLElement` and fails loudly when the split suite
 * would otherwise continue with a bad panel/query assumption.
 */
export function expectHtmlElement(node: Element | null): HTMLElement {
  expect(node).toBeInstanceOf(HTMLElement)
  return node as HTMLElement
}

/**
 * Seeds the interactive schedule review fixture used by shell route tests that depend
 * on PME output rather than the bare schedule status snapshot.
 */
export function seedInteractiveSchedule() {
  backendTestHarness.seedSchedule(
    {
      platform: 'macos',
      label: 'com.yi-ting.pathkeep.backup',
      executablePath: '/Applications/PathKeep.app',
      generatedFiles: [
        {
          relativePath: 'schedule/com.yi-ting.pathkeep.backup.plist',
          absolutePath:
            '/Users/test/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist',
          purpose: 'LaunchAgent plist',
          contents:
            '<?xml version="1.0"?><plist><dict><key>Label</key><string>com.yi-ting.pathkeep.backup</string></dict></plist>',
        },
      ],
      manualSteps: ['Review the LaunchAgent install.'],
      applyCommands: [['launchctl', 'bootstrap']],
      rollbackCommands: [['launchctl', 'bootout']],
      applySupported: true,
    },
    {
      platform: 'macos',
      label: 'com.yi-ting.pathkeep.backup',
      dueAfterHours: 72,
      checkIntervalHours: 6,
      applySupported: true,
      installState: 'installed',
      detectedFiles: [
        '~/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist',
      ],
      manualSteps: ['Remove the LaunchAgent if you no longer want automation.'],
      auditPath: null,
      lastSuccessfulBackupAt: null,
      warnings: [],
    },
  )
}
