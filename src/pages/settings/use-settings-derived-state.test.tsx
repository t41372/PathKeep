/**
 * @file use-settings-derived-state.test.tsx
 * @description Hook-level coverage for Settings derived-state runtime and search-rule workflows.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Verify deterministic runtime load, rebuild, clear, retry, cancel, search-rule CRUD, and module/plugin toggles.
 * - Protect Settings from reporting a healthy derived-state surface when backend runtime/search-rule calls fail.
 * - Keep behavior tests close to the hook that owns these workflows.
 *
 * ## Not responsible for
 * - Re-testing derived-state section markup.
 * - Re-testing Core Intelligence backend command implementation details.
 *
 * ## Dependencies
 * - Uses shipped i18n, preview snapshot fixtures, backend-client spies, and a mocked rebuild queue API.
 *
 * ## Performance notes
 * - Hook-level tests cover the native-command state machine without mounting the large Maintenance route.
 */

import { act, renderHook, waitFor } from '@testing-library/react'
import type { ReactNode } from 'react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { mockSnapshot } from '../../lib/backend-preview-fixtures'
import * as coreIntelligenceApi from '../../lib/core-intelligence/api'
import type * as coreIntelligenceApiModule from '../../lib/core-intelligence/api'
import type {
  CoreIntelligenceQueueReport,
  SearchEngineRule,
} from '../../lib/core-intelligence/types'
import { I18nProvider } from '../../lib/i18n'
import type {
  AppConfig,
  AppSnapshot,
  ClearDerivedIntelligenceReport,
  IntelligenceRuntimeSnapshot,
} from '../../lib/types'
import { useSettingsDerivedState } from './use-settings-derived-state'

vi.mock('../../lib/core-intelligence/api', async (importOriginal) => {
  const actual = await importOriginal<typeof coreIntelligenceApiModule>()
  return {
    ...actual,
    queueCoreIntelligenceRebuild: vi.fn(),
    clearIntelligenceOverviewCache: vi.fn(),
  }
})

function Wrapper({ children }: { children: ReactNode }) {
  return <I18nProvider>{children}</I18nProvider>
}

describe('useSettingsDerivedState', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    vi.mocked(
      coreIntelligenceApi.queueCoreIntelligenceRebuild,
    ).mockResolvedValue(queueReportFixture())
  })

  test('loads runtime/search rules and executes rebuild, clear, retry, cancel, and rule CRUD', async () => {
    const snapshot = snapshotFixture()
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    const loadRuntime = vi
      .spyOn(backend, 'loadIntelligenceRuntime')
      .mockResolvedValue(runtimeFixture())
    const listRules = vi
      .spyOn(backend, 'listSearchEngineRules')
      .mockResolvedValue([searchRuleFixture()])
    const clearDerived = vi
      .spyOn(backend, 'clearDerivedIntelligence')
      .mockResolvedValue(clearReportFixture())
    const upsertRule = vi
      .spyOn(backend, 'upsertSearchEngineRule')
      .mockResolvedValue([searchRuleFixture({ displayName: 'MDN Search' })])
    const deleteRule = vi
      .spyOn(backend, 'deleteSearchEngineRule')
      .mockResolvedValue([])
    const retryJob = vi
      .spyOn(backend, 'retryIntelligenceJob')
      .mockResolvedValue(runtimeFixture({ queued: 2 }))
    const cancelJob = vi
      .spyOn(backend, 'cancelIntelligenceJob')
      .mockResolvedValue(runtimeFixture({ cancelled: 1 }))

    const { result } = renderHook(
      () =>
        useSettingsDerivedState({
          dashboard: null,
          refreshAppData,
          refreshKey: 1,
          saveConfig: vi.fn((config: AppConfig) =>
            Promise.resolve({
              ...snapshot,
              config,
            }),
          ),
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    await waitFor(() =>
      expect(result.current.derived.intelligenceRuntime?.queue.queued).toBe(1),
    )
    await waitFor(() =>
      expect(result.current.derived.searchEngineRules).toHaveLength(1),
    )
    expect(listRules).toHaveBeenCalledTimes(1)

    await act(async () => {
      await result.current.derived.onRebuildDerivedState()
    })
    expect(
      coreIntelligenceApi.queueCoreIntelligenceRebuild,
    ).toHaveBeenCalledWith({ fullRebuild: true })
    expect(result.current.derived.rebuildQueueReport?.jobId).toBe(77)
    expect(refreshAppData).toHaveBeenCalledTimes(1)
    // A rebuild invalidates the cached intelligence overview so the next
    // /intelligence visit re-fetches instead of showing the pre-rebuild data.
    expect(
      coreIntelligenceApi.clearIntelligenceOverviewCache,
    ).toHaveBeenCalled()

    await act(async () => {
      await result.current.derived.onClearDerivedState()
    })
    expect(clearDerived).toHaveBeenCalledTimes(1)
    expect(result.current.derived.clearReport?.clearedStructuralRows).toBe(4)

    loadRuntime.mockRejectedValueOnce(new Error('runtime refresh failed'))
    await act(async () => {
      await result.current.derived.onClearDerivedState()
    })
    expect(result.current.derived.intelligenceRuntimeError).toBe(
      'runtime refresh failed',
    )

    loadRuntime.mockRejectedValueOnce('runtime refresh fallback')
    await act(async () => {
      await result.current.derived.onClearDerivedState()
    })
    expect(result.current.derived.intelligenceRuntimeError).toBe(
      'runtime refresh fallback',
    )

    act(() => {
      result.current.derived.onSearchEngineRuleDraftChange({
        displayName: 'Ignored without a draft',
      })
    })
    expect(result.current.derived.searchEngineRuleDraft).toBeNull()

    act(() => {
      result.current.derived.onStartSearchEngineRule()
    })
    expect(result.current.derived.searchEngineRuleDraftValid).toBe(false)
    await act(async () => {
      await result.current.derived.onSaveSearchEngineRule()
    })
    expect(upsertRule).not.toHaveBeenCalled()

    act(() => {
      result.current.derived.onSearchEngineRuleDraftChange({
        displayName: ' MDN Search ',
        engineId: ' mdn ',
        hostPattern: ' developer.mozilla.org ',
        pathPrefix: ' /search ',
        queryParamKey: ' q ',
        note: ' docs ',
        exampleUrl: ' https://developer.mozilla.org/search?q=rust ',
      })
    })
    expect(result.current.derived.searchEngineRuleDraftValid).toBe(true)

    await act(async () => {
      await result.current.derived.onSaveSearchEngineRule()
    })
    expect(upsertRule).toHaveBeenCalledWith(
      expect.objectContaining({
        displayName: 'MDN Search',
        engineId: 'mdn',
        hostPattern: 'developer.mozilla.org',
        pathPrefix: '/search',
        queryParamKey: 'q',
      }),
    )
    expect(result.current.derived.searchEngineRuleDraft).toBeNull()

    act(() => {
      result.current.derived.onEditSearchEngineRule(searchRuleFixture())
    })
    expect(result.current.derived.searchEngineRuleDraft?.ruleId).toBe(
      'custom:docs',
    )
    act(() => {
      result.current.derived.onCancelSearchEngineRuleEdit()
    })
    expect(result.current.derived.searchEngineRuleDraft).toBeNull()

    act(() => {
      result.current.derived.onEditSearchEngineRule(searchRuleFixture())
    })
    await act(async () => {
      await result.current.derived.onDeleteSearchEngineRule('custom:docs')
    })
    expect(deleteRule).toHaveBeenCalledWith('custom:docs')
    expect(result.current.derived.searchEngineRuleDraft).toBeNull()

    await act(async () => {
      await result.current.derived.onRetryRuntimeJob(10)
    })
    expect(retryJob).toHaveBeenCalledWith(10)
    expect(result.current.derived.intelligenceRuntime?.queue.queued).toBe(2)

    await act(async () => {
      await result.current.derived.onCancelRuntimeJob(10)
    })
    expect(cancelJob).toHaveBeenCalledWith(10)
    expect(result.current.derived.intelligenceRuntime?.queue.cancelled).toBe(1)

    expect(loadRuntime).toHaveBeenCalled()
  })

  test('persists enrichment plugin and deterministic module toggles', async () => {
    const snapshot = snapshotFixture()
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    const saveConfig = vi.fn((config: AppConfig) =>
      Promise.resolve({
        ...snapshot,
        config,
      }),
    )
    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      runtimeFixture(),
    )
    vi.spyOn(backend, 'listSearchEngineRules').mockResolvedValue([])

    const { result } = renderHook(
      () =>
        useSettingsDerivedState({
          dashboard: null,
          refreshAppData,
          refreshKey: 1,
          saveConfig,
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    await waitFor(() =>
      expect(result.current.derived.intelligenceRuntime).not.toBeNull(),
    )

    await act(async () => {
      await result.current.derived.onEnrichmentPluginToggle(
        'title-normalization',
      )
    })
    expect(saveConfig).toHaveBeenLastCalledWith(
      expect.objectContaining({
        enrichment: expect.objectContaining({
          plugins: expect.arrayContaining([
            expect.objectContaining({
              id: 'title-normalization',
              enabled: false,
            }),
          ]),
        }),
        ai: expect.objectContaining({
          enrichmentPlugins: expect.arrayContaining([
            expect.objectContaining({
              pluginId: 'title-normalization',
              enabled: false,
            }),
          ]),
        }),
      }),
    )

    await act(async () => {
      await result.current.derived.onDeterministicModuleToggle('sessions')
    })
    expect(saveConfig).toHaveBeenLastCalledWith(
      expect.objectContaining({
        deterministic: {
          modules: expect.arrayContaining([
            expect.objectContaining({
              id: 'sessions',
              enabled: false,
            }),
          ]),
        },
      }),
    )

    await act(async () => {
      await result.current.derived.onDeterministicModuleToggle('fresh-module')
    })
    expect(saveConfig).toHaveBeenLastCalledWith(
      expect.objectContaining({
        deterministic: {
          modules: expect.arrayContaining([
            expect.objectContaining({
              id: 'fresh-module',
              enabled: false,
            }),
          ]),
        },
      }),
    )
    expect(refreshAppData).toHaveBeenCalledTimes(3)
  })

  test('handles disabled mode and runtime/search-rule load failures', async () => {
    const snapshot = snapshotFixture()
    const loadRuntime = vi
      .spyOn(backend, 'loadIntelligenceRuntime')
      .mockRejectedValue(new Error('runtime down'))
    const listRules = vi
      .spyOn(backend, 'listSearchEngineRules')
      .mockRejectedValue(new Error('rules unavailable'))
    const upsertRule = vi.spyOn(backend, 'upsertSearchEngineRule')

    const { result, rerender } = renderHook(
      ({ enabled }: { enabled: boolean }) =>
        useSettingsDerivedState({
          dashboard: null,
          enabled,
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          refreshKey: 1,
          saveConfig: vi.fn((config: AppConfig) =>
            Promise.resolve({
              ...snapshot,
              config,
            }),
          ),
          snapshot,
        }),
      {
        initialProps: { enabled: true },
        wrapper: Wrapper,
      },
    )

    await waitFor(() =>
      expect(result.current.derived.intelligenceRuntimeError).toBe(
        'runtime down',
      ),
    )
    await waitFor(() =>
      expect(result.current.derived.searchEngineRuleError).toBe(
        'rules unavailable',
      ),
    )

    rerender({ enabled: false })
    await waitFor(() =>
      expect(result.current.derived.intelligenceRuntimeError).toBeNull(),
    )
    expect(result.current.derived.searchEngineRules).toEqual([])

    await act(async () => {
      await result.current.derived.onRebuildDerivedState()
      await result.current.derived.onClearDerivedState()
      result.current.derived.onStartSearchEngineRule()
      result.current.derived.onEditSearchEngineRule(searchRuleFixture())
      await result.current.derived.onSaveSearchEngineRule()
      await result.current.derived.onDeleteSearchEngineRule('custom:docs')
      await result.current.derived.onRetryRuntimeJob(10)
      await result.current.derived.onCancelRuntimeJob(10)
      await result.current.derived.onEnrichmentPluginToggle(
        'title-normalization',
      )
      await result.current.derived.onDeterministicModuleToggle('sessions')
    })

    expect(upsertRule).not.toHaveBeenCalled()
    expect(loadRuntime).toHaveBeenCalledTimes(1)
    expect(listRules).toHaveBeenCalledTimes(1)
  })

  test('uses fallback copy for non-Error initial runtime/search-rule load failures', async () => {
    const snapshot = snapshotFixture()
    vi.spyOn(backend, 'loadIntelligenceRuntime').mockRejectedValue(
      'runtime fallback',
    )
    vi.spyOn(backend, 'listSearchEngineRules').mockRejectedValue(
      'rules fallback',
    )

    const { result } = renderHook(
      () =>
        useSettingsDerivedState({
          dashboard: null,
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          refreshKey: 1,
          saveConfig: vi.fn((config: AppConfig) =>
            Promise.resolve({
              ...snapshot,
              config,
            }),
          ),
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    await waitFor(() =>
      expect(result.current.derived.intelligenceRuntimeError).toBe(
        'runtime fallback',
      ),
    )
    await waitFor(() =>
      expect(result.current.derived.searchEngineRuleError).toBe(
        'rules fallback',
      ),
    )
  })

  test('surfaces search-rule action failures and noops when snapshot is absent', async () => {
    const snapshot = snapshotFixture()
    const saveConfig = vi.fn((config: AppConfig) =>
      Promise.resolve({
        ...snapshot,
        config,
      }),
    )
    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      runtimeFixture(),
    )
    vi.spyOn(backend, 'listSearchEngineRules').mockResolvedValue([
      searchRuleFixture(),
    ])
    vi.spyOn(backend, 'upsertSearchEngineRule').mockRejectedValue(
      new Error('save rule failed'),
    )
    vi.spyOn(backend, 'deleteSearchEngineRule').mockRejectedValue(
      new Error('delete rule failed'),
    )

    const { result } = renderHook(
      () =>
        useSettingsDerivedState({
          dashboard: null,
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          refreshKey: 1,
          saveConfig,
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    await waitFor(() =>
      expect(result.current.derived.searchEngineRules).toHaveLength(1),
    )

    act(() => {
      result.current.derived.onStartSearchEngineRule()
      result.current.derived.onSearchEngineRuleDraftChange({
        displayName: 'Docs',
        engineId: 'docs',
        hostPattern: 'docs.example.com',
        queryParamKey: 'q',
      })
    })
    await act(async () => {
      await result.current.derived.onSaveSearchEngineRule()
    })
    expect(result.current.derived.searchEngineRuleError).toBe(
      'save rule failed',
    )

    act(() => {
      result.current.derived.onEditSearchEngineRule(searchRuleFixture())
    })
    await act(async () => {
      await result.current.derived.onDeleteSearchEngineRule('custom:docs')
    })
    expect(result.current.derived.searchEngineRuleError).toBe(
      'delete rule failed',
    )

    const nullSnapshot = renderHook(
      () =>
        useSettingsDerivedState({
          dashboard: null,
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          refreshKey: 1,
          saveConfig,
          snapshot: null,
        }),
      { wrapper: Wrapper },
    )

    await act(async () => {
      await nullSnapshot.result.current.derived.onEnrichmentPluginToggle(
        'title-normalization',
      )
      await nullSnapshot.result.current.derived.onDeterministicModuleToggle(
        'sessions',
      )
    })
    expect(saveConfig).not.toHaveBeenCalled()
  })

  test('uses fallback copy for non-Error search-rule failures', async () => {
    const snapshot = snapshotFixture()
    vi.spyOn(backend, 'loadIntelligenceRuntime').mockResolvedValue(
      runtimeFixture(),
    )
    vi.spyOn(backend, 'listSearchEngineRules').mockResolvedValue([
      searchRuleFixture(),
    ])
    vi.spyOn(backend, 'upsertSearchEngineRule').mockRejectedValue(
      'save fallback',
    )
    vi.spyOn(backend, 'deleteSearchEngineRule').mockRejectedValue(
      'delete fallback',
    )

    const { result } = renderHook(
      () =>
        useSettingsDerivedState({
          dashboard: null,
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          refreshKey: 1,
          saveConfig: vi.fn((config: AppConfig) =>
            Promise.resolve({
              ...snapshot,
              config,
            }),
          ),
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    await waitFor(() =>
      expect(result.current.derived.searchEngineRules).toHaveLength(1),
    )

    act(() => {
      result.current.derived.onStartSearchEngineRule()
      result.current.derived.onSearchEngineRuleDraftChange({
        displayName: 'Docs',
        engineId: 'docs',
        hostPattern: 'docs.example.com',
        queryParamKey: 'q',
      })
    })
    await act(async () => {
      await result.current.derived.onSaveSearchEngineRule()
    })
    expect(result.current.derived.searchEngineRuleError).toBe('save fallback')

    act(() => {
      result.current.derived.onEditSearchEngineRule(searchRuleFixture())
    })
    await act(async () => {
      await result.current.derived.onDeleteSearchEngineRule('custom:docs')
    })
    expect(result.current.derived.searchEngineRuleError).toBe('delete fallback')
  })

  test('ignores late runtime and search-rule loads after unmount', async () => {
    const snapshot = snapshotFixture()
    const runtime = deferred<IntelligenceRuntimeSnapshot>()
    const rules = deferred<SearchEngineRule[]>()
    vi.spyOn(backend, 'loadIntelligenceRuntime').mockReturnValue(
      runtime.promise,
    )
    vi.spyOn(backend, 'listSearchEngineRules').mockReturnValue(rules.promise)

    const { unmount } = renderHook(
      () =>
        useSettingsDerivedState({
          dashboard: null,
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          refreshKey: 1,
          saveConfig: vi.fn((config: AppConfig) =>
            Promise.resolve({
              ...snapshot,
              config,
            }),
          ),
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    unmount()

    await act(async () => {
      runtime.reject(new Error('late runtime failure'))
      rules.reject(new Error('late rules failure'))
      await Promise.resolve()
      await Promise.resolve()
    })

    const successRuntime = deferred<IntelligenceRuntimeSnapshot>()
    const successRules = deferred<SearchEngineRule[]>()
    vi.spyOn(backend, 'loadIntelligenceRuntime').mockReturnValue(
      successRuntime.promise,
    )
    vi.spyOn(backend, 'listSearchEngineRules').mockReturnValue(
      successRules.promise,
    )

    const successHook = renderHook(
      () =>
        useSettingsDerivedState({
          dashboard: null,
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          refreshKey: 2,
          saveConfig: vi.fn((config: AppConfig) =>
            Promise.resolve({
              ...snapshot,
              config,
            }),
          ),
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    successHook.unmount()
    await act(async () => {
      successRuntime.resolve(runtimeFixture())
      successRules.resolve([searchRuleFixture()])
      await successRuntime.promise
      await successRules.promise
    })
  })
})

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, reject, resolve }
}

function snapshotFixture(): AppSnapshot {
  const snapshot = structuredClone(mockSnapshot)
  return {
    ...snapshot,
    config: {
      ...snapshot.config,
      initialized: true,
    },
    archiveStatus: {
      ...snapshot.archiveStatus,
      initialized: true,
      unlocked: true,
    },
  }
}

function runtimeFixture(
  queueOverrides: Partial<IntelligenceRuntimeSnapshot['queue']> = {},
): IntelligenceRuntimeSnapshot {
  return {
    queue: {
      queued: 1,
      running: 0,
      succeeded: 2,
      failed: 0,
      cancelled: 0,
      lastActivityAt: '2026-04-25T10:00:00Z',
      ...queueOverrides,
    },
    plugins: [
      {
        pluginId: 'title-normalization',
        sourceKind: 'local',
        enabled: true,
        storedRecords: 10,
        queuedJobs: 0,
        runningJobs: 0,
        failedJobs: 0,
        lastCompletedAt: '2026-04-25T09:55:00Z',
        lastError: null,
      },
    ],
    modules: [
      {
        moduleId: 'sessions',
        enabled: true,
        version: 'ci-v1',
        status: 'ready',
        dependsOn: ['visit-derived-facts'],
        derivedTables: ['sessions'],
        lastRunId: 12,
        lastBuiltAt: '2026-04-25T09:50:00Z',
        lastInvalidatedAt: null,
        staleReason: null,
        notes: [],
      },
    ],
    recentJobs: [],
    notes: [],
  }
}

function searchRuleFixture(
  overrides: Partial<SearchEngineRule> = {},
): SearchEngineRule {
  return {
    ruleId: 'custom:docs',
    engineId: 'docs',
    displayName: 'Docs Search',
    hostPattern: 'docs.example.com',
    pathPrefix: '/search',
    queryParamKey: 'q',
    enabled: true,
    note: 'Docs search',
    exampleUrl: 'https://docs.example.com/search?q=sqlite',
    builtIn: false,
    ...overrides,
  }
}

function queueReportFixture(): CoreIntelligenceQueueReport {
  return {
    jobId: 77,
    state: 'queued',
    notes: ['queued from settings'],
  }
}

function clearReportFixture(): ClearDerivedIntelligenceReport {
  return {
    clearedVisitDerivedFactRows: 1,
    clearedDailyRollupRows: 2,
    clearedStructuralRows: 4,
    clearedRuntimeRows: 8,
    notes: ['cleared derived tables'],
  }
}
