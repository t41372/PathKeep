/**
 * @file use-settings-derived-state.ts
 * @description Owns Settings derived-state runtime, search-engine rules, and rebuild/clear workflows.
 * @module pages/settings
 *
 * ## 職責
 * - 載入 derived runtime snapshot 與 search-engine rules。
 * - 管理 rebuild / clear / retry / cancel 與 search-rule CRUD handlers。
 * - 對 derived-state section 暴露 route-owned runtime review state。
 *
 * ## 不負責
 * - 不渲染 derived-state section UI。
 * - 不管理 AI provider draft 或 remote backup。
 * - 不建立新的 shared runtime polling source。
 *
 * ## 依賴關係
 * - 依賴 shell snapshot、dashboard、`backend-client`、與 core-intelligence API。
 *
 * ## 性能備注
 * - 只在 shell `refreshKey` 或 archive unlock/config state 變動時重新同步 runtime/search rules。
 */

import { useEffect, useMemo, useState } from 'react'
import { backend } from '../../lib/backend-client'
import { queueCoreIntelligenceRebuild } from '../../lib/core-intelligence/api'
import type {
  CoreIntelligenceQueueReport,
  SearchEngineRule,
  SearchEngineRuleInput,
} from '../../lib/core-intelligence/types'
import {
  enrichmentPluginState,
  resolveEnrichmentSettings,
} from '../../lib/enrichment'
import { useI18n } from '../../lib/i18n'
import {
  upsertDeterministicModuleState,
  upsertEnrichmentPluginPreference,
} from '../../lib/intelligence-runtime'
import type {
  AppConfig,
  AppSnapshot,
  ClearDerivedIntelligenceReport,
  DashboardSnapshot,
  IntelligenceRuntimeSnapshot,
} from '../../lib/types'
import {
  buildSearchEngineRuleDraft,
  normalizeSearchEngineRuleDraft,
} from './helpers'

interface UseSettingsDerivedStateArgs {
  dashboard: DashboardSnapshot | null
  enabled?: boolean
  refreshAppData: () => Promise<void>
  refreshKey: number
  saveConfig: (config: AppConfig) => Promise<AppSnapshot>
  snapshot: AppSnapshot | null
}

/**
 * Keeps derived-state runtime and search-rule workflows under one focused hook.
 */
export function useSettingsDerivedState({
  dashboard,
  enabled = true,
  refreshAppData,
  refreshKey,
  saveConfig,
  snapshot,
}: UseSettingsDerivedStateArgs) {
  const { t, ns } = useI18n()
  const settingsNs = ns('settings')
  const [rebuildQueueReport, setRebuildQueueReport] =
    useState<CoreIntelligenceQueueReport | null>(null)
  const [clearReport, setClearReport] =
    useState<ClearDerivedIntelligenceReport | null>(null)
  const [derivedAction, setDerivedAction] = useState<string | null>(null)
  const [intelligenceRuntime, setIntelligenceRuntime] =
    useState<IntelligenceRuntimeSnapshot | null>(null)
  const [intelligenceRuntimeError, setIntelligenceRuntimeError] = useState<
    string | null
  >(null)
  const [searchEngineRules, setSearchEngineRules] = useState<
    SearchEngineRule[]
  >([])
  const [searchEngineRulesLoading, setSearchEngineRulesLoading] =
    useState(false)
  const [searchEngineRuleDraft, setSearchEngineRuleDraft] =
    useState<SearchEngineRuleInput | null>(null)
  const [searchEngineRuleError, setSearchEngineRuleError] = useState<
    string | null
  >(null)
  const enrichmentSettings = useMemo(
    () => resolveEnrichmentSettings(snapshot?.config.enrichment),
    [snapshot?.config.enrichment],
  )

  async function refreshIntelligenceRuntimeState() {
    try {
      const runtime = await backend.loadIntelligenceRuntime()
      setIntelligenceRuntime(runtime)
      setIntelligenceRuntimeError(null)
    } catch (error) {
      setIntelligenceRuntime(null)
      setIntelligenceRuntimeError(
        error instanceof Error ? error.message : t('common.notAvailable'),
      )
    }
  }

  useEffect(() => {
    let cancelled = false

    const loadRuntime = async () => {
      if (!enabled) {
        setIntelligenceRuntime(null)
        setIntelligenceRuntimeError(null)
        setRebuildQueueReport(null)
        setClearReport(null)
        return
      }

      try {
        const runtime = await backend.loadIntelligenceRuntime()
        if (!cancelled) {
          setIntelligenceRuntime(runtime)
          setIntelligenceRuntimeError(null)
        }
      } catch (error) {
        if (!cancelled) {
          setIntelligenceRuntime(null)
          setIntelligenceRuntimeError(
            error instanceof Error ? error.message : t('common.notAvailable'),
          )
        }
      }
    }

    void loadRuntime()
    return () => {
      cancelled = true
    }
  }, [enabled, refreshKey, snapshot?.config.initialized, t])

  useEffect(() => {
    let cancelled = false

    if (
      !enabled ||
      !snapshot?.config.initialized ||
      !snapshot.archiveStatus.unlocked
    ) {
      setSearchEngineRules([])
      setSearchEngineRuleDraft(null)
      setSearchEngineRuleError(null)
      setSearchEngineRulesLoading(false)
      return
    }

    setSearchEngineRulesLoading(true)

    const loadSearchEngineRules = async () => {
      try {
        const rules = await backend.listSearchEngineRules()
        if (!cancelled) {
          setSearchEngineRules(rules)
          setSearchEngineRuleError(null)
        }
      } catch (error) {
        if (!cancelled) {
          setSearchEngineRules([])
          setSearchEngineRuleError(
            error instanceof Error ? error.message : t('common.notAvailable'),
          )
        }
      } finally {
        if (!cancelled) {
          setSearchEngineRulesLoading(false)
        }
      }
    }

    void loadSearchEngineRules()
    return () => {
      cancelled = true
    }
  }, [
    refreshKey,
    enabled,
    snapshot?.archiveStatus.unlocked,
    snapshot?.config.initialized,
    t,
  ])

  const searchEngineRuleDraftValid = useMemo(() => {
    if (!searchEngineRuleDraft) {
      return false
    }
    const normalized = normalizeSearchEngineRuleDraft(searchEngineRuleDraft)
    return Boolean(
      normalized.engineId &&
      normalized.displayName &&
      normalized.hostPattern &&
      normalized.queryParamKey,
    )
  }, [searchEngineRuleDraft])

  async function handleRebuildDerivedState() {
    if (!enabled) {
      return
    }

    setDerivedAction(t('settings.rebuildingDerivedState'))
    try {
      const report = await queueCoreIntelligenceRebuild({ fullRebuild: true })
      setRebuildQueueReport(report)
      setClearReport(null)
      await refreshAppData()
      await refreshIntelligenceRuntimeState()
    } finally {
      setDerivedAction(null)
    }
  }

  async function handleClearDerivedState() {
    if (!enabled) {
      return
    }

    setDerivedAction(t('settings.clearingDerivedState'))
    try {
      const report = await backend.clearDerivedIntelligence()
      setClearReport(report)
      setRebuildQueueReport(null)
      await refreshAppData()
      await refreshIntelligenceRuntimeState()
    } finally {
      setDerivedAction(null)
    }
  }

  function handleStartSearchEngineRule() {
    if (!enabled) {
      return
    }

    setSearchEngineRuleDraft(buildSearchEngineRuleDraft())
  }

  function handleEditSearchEngineRule(rule: SearchEngineRule) {
    if (!enabled) {
      return
    }

    setSearchEngineRuleDraft(buildSearchEngineRuleDraft(rule))
  }

  function handleSearchEngineRuleDraftChange(
    patch: Partial<SearchEngineRuleInput>,
  ) {
    setSearchEngineRuleDraft((current) =>
      current ? { ...current, ...patch } : current,
    )
  }

  function handleCancelSearchEngineRuleEdit() {
    setSearchEngineRuleDraft(null)
  }

  async function handleSaveSearchEngineRule() {
    if (!enabled) {
      return
    }

    if (!searchEngineRuleDraft || !searchEngineRuleDraftValid) {
      return
    }

    setDerivedAction(settingsNs('searchRulesSaving'))
    try {
      const rules = await backend.upsertSearchEngineRule(
        normalizeSearchEngineRuleDraft(searchEngineRuleDraft),
      )
      setSearchEngineRules(rules)
      setSearchEngineRuleDraft(null)
      setSearchEngineRuleError(null)
      const report = await queueCoreIntelligenceRebuild({ fullRebuild: true })
      setRebuildQueueReport(report)
      setClearReport(null)
      await refreshAppData()
      await refreshIntelligenceRuntimeState()
    } catch (error) {
      setSearchEngineRuleError(
        error instanceof Error ? error.message : t('common.notAvailable'),
      )
    } finally {
      setDerivedAction(null)
    }
  }

  async function handleDeleteSearchEngineRule(ruleId: string) {
    if (!enabled) {
      return
    }

    setDerivedAction(settingsNs('searchRulesDeleting'))
    try {
      const rules = await backend.deleteSearchEngineRule(ruleId)
      setSearchEngineRules(rules)
      if (searchEngineRuleDraft?.ruleId === ruleId) {
        setSearchEngineRuleDraft(null)
      }
      setSearchEngineRuleError(null)
      const report = await queueCoreIntelligenceRebuild({ fullRebuild: true })
      setRebuildQueueReport(report)
      setClearReport(null)
      await refreshAppData()
      await refreshIntelligenceRuntimeState()
    } catch (error) {
      setSearchEngineRuleError(
        error instanceof Error ? error.message : t('common.notAvailable'),
      )
    } finally {
      setDerivedAction(null)
    }
  }

  async function handleRetryIntelligenceRuntimeJob(jobId: number) {
    if (!enabled) {
      return
    }

    setDerivedAction(settingsNs('retryRuntimeJob'))
    try {
      const runtime = await backend.retryIntelligenceJob(jobId)
      setIntelligenceRuntime(runtime)
      setIntelligenceRuntimeError(null)
    } finally {
      setDerivedAction(null)
    }
  }

  async function handleCancelIntelligenceRuntimeJob(jobId: number) {
    if (!enabled) {
      return
    }

    setDerivedAction(settingsNs('cancelRuntimeJob'))
    try {
      const runtime = await backend.cancelIntelligenceJob(jobId)
      setIntelligenceRuntime(runtime)
      setIntelligenceRuntimeError(null)
    } finally {
      setDerivedAction(null)
    }
  }

  async function handleEnrichmentPluginToggle(pluginId: string) {
    if (!enabled) {
      return
    }

    if (!snapshot) {
      return
    }

    const currentPlugin = enrichmentPluginState(enrichmentSettings, pluginId)
    const nextEnabled = !currentPlugin.enabled
    const nextPlugins = resolveEnrichmentSettings(
      snapshot.config.enrichment,
    ).plugins.map((plugin) =>
      plugin.id === pluginId ? { ...plugin, enabled: nextEnabled } : plugin,
    )
    setDerivedAction(t('settings.savingEnrichmentSettings'))
    try {
      await saveConfig({
        ...snapshot.config,
        enrichment: { plugins: nextPlugins },
        ai: {
          ...snapshot.config.ai,
          enrichmentPlugins: upsertEnrichmentPluginPreference(
            snapshot.config.ai.enrichmentPlugins,
            pluginId,
            nextEnabled,
          ),
        },
      })
      await refreshAppData()
      await refreshIntelligenceRuntimeState()
    } finally {
      setDerivedAction(null)
    }
  }

  async function handleDeterministicModuleToggle(moduleId: string) {
    if (!enabled) {
      return
    }

    if (!snapshot) {
      return
    }

    const currentModule =
      snapshot.config.deterministic.modules.find(
        (module) => module.id === moduleId,
      ) ?? null
    const nextEnabled = !(currentModule?.enabled ?? true)
    setDerivedAction(t('settings.savingDeterministicModules'))
    try {
      await saveConfig({
        ...snapshot.config,
        deterministic: {
          modules: upsertDeterministicModuleState(
            snapshot.config.deterministic.modules,
            moduleId,
            nextEnabled,
          ),
        },
      })
      await refreshAppData()
      await refreshIntelligenceRuntimeState()
    } finally {
      setDerivedAction(null)
    }
  }

  return {
    derived: {
      action: derivedAction,
      clearReport,
      dashboardRecentRun: dashboard?.recentRuns[0] ?? null,
      intelligenceRuntime,
      intelligenceRuntimeError,
      rebuildQueueReport,
      searchEngineRuleDraft,
      searchEngineRuleDraftValid,
      searchEngineRuleError,
      searchEngineRules,
      searchEngineRulesLoading,
      onCancelRuntimeJob: handleCancelIntelligenceRuntimeJob,
      onCancelSearchEngineRuleEdit: handleCancelSearchEngineRuleEdit,
      onClearDerivedState: handleClearDerivedState,
      onDeleteSearchEngineRule: handleDeleteSearchEngineRule,
      onDeterministicModuleToggle: handleDeterministicModuleToggle,
      onEditSearchEngineRule: handleEditSearchEngineRule,
      onEnrichmentPluginToggle: handleEnrichmentPluginToggle,
      onRebuildDerivedState: handleRebuildDerivedState,
      onRetryRuntimeJob: handleRetryIntelligenceRuntimeJob,
      onSaveSearchEngineRule: handleSaveSearchEngineRule,
      onSearchEngineRuleDraftChange: handleSearchEngineRuleDraftChange,
      onStartSearchEngineRule: handleStartSearchEngineRule,
    },
  }
}
