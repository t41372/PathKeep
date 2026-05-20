/**
 * @file derived-state-section.tsx
 * @description Renders the Settings derived-state shell and composes the extracted search-rule and runtime review surfaces.
 * @module pages/settings
 *
 * ## 職責
 * - 呈現 derived-only boundary callouts 與 top-level rebuild/clear actions。
 * - 組合 search-rule review 與 runtime review 兩個 extracted sub-surfaces。
 * - 保持 derived-state panel 的 anchor / badge / boundary copy 穩定。
 *
 * ## 不負責
 * - 不持有 derived runtime state。
 * - 不直接執行 rebuild / clear / search-rule mutation。
 * - 不重新定義 runtime queue grammar。
 *
 * ## 依賴關係
 * - 依賴 derived-state route state 與 extracted subcomponents。
 *
 * ## 性能備注
 * - 本模組只做 composition，不做額外資料查詢。
 */

import {
  PaperCard,
  PaperCardBadge,
  PaperCardBody,
  PaperCardHeader,
} from '@/components/cards'
import { StatusCallout } from '../../components/primitives/status-callout'
import type {
  AppSnapshot,
  ClearDerivedIntelligenceReport,
  DashboardSnapshot,
  IntelligenceRuntimeSnapshot,
} from '../../lib/types'
import { useI18n } from '../../lib/i18n'
import type {
  CoreIntelligenceQueueReport,
  SearchEngineRule,
  SearchEngineRuleInput,
} from '../../lib/core-intelligence/types'
import { DerivedRuntimeReview } from './derived-runtime-review'
import { DerivedSearchRulesReview } from './derived-search-rules-review'
import type { SettingsSectionNavItem } from './section-nav-items'

/**
 * Defines the route-owned derived-state payload consumed by the section shell.
 */
export interface DerivedStateSectionState {
  action: string | null
  clearReport: ClearDerivedIntelligenceReport | null
  dashboardRecentRun: DashboardSnapshot['recentRuns'][number] | null
  intelligenceRuntime: IntelligenceRuntimeSnapshot | null
  intelligenceRuntimeError: string | null
  rebuildQueueReport: CoreIntelligenceQueueReport | null
  searchEngineRuleDraft: SearchEngineRuleInput | null
  searchEngineRuleDraftValid: boolean
  searchEngineRuleError: string | null
  searchEngineRules: SearchEngineRule[]
  searchEngineRulesLoading: boolean
  onCancelRuntimeJob: (jobId: number) => Promise<void>
  onCancelSearchEngineRuleEdit: () => void
  onClearDerivedState: () => Promise<void>
  onDeleteSearchEngineRule: (ruleId: string) => Promise<void>
  onDeterministicModuleToggle: (moduleId: string) => Promise<void>
  onEditSearchEngineRule: (rule: SearchEngineRule) => void
  onEnrichmentPluginToggle: (pluginId: string) => Promise<void>
  onRebuildDerivedState: () => Promise<void>
  onRetryRuntimeJob: (jobId: number) => Promise<void>
  onSaveSearchEngineRule: () => Promise<void>
  onSearchEngineRuleDraftChange: (patch: Partial<SearchEngineRuleInput>) => void
  onStartSearchEngineRule: () => void
}

/**
 * Groups the stable section anchor descriptor with the derived-state route payload.
 */
export interface DerivedStateSectionProps {
  navItem: SettingsSectionNavItem
  snapshot: AppSnapshot | null
  state: DerivedStateSectionState
}

/**
 * Renders the derived-state section shell from route-owned state.
 */
export function DerivedStateSection({
  navItem,
  snapshot,
  state,
}: DerivedStateSectionProps) {
  const { t, ns } = useI18n()
  const settingsNs = ns('settings')

  if (!snapshot) {
    return null
  }

  return (
    <PaperCard testId={navItem.id}>
      <span id={navItem.id} aria-hidden />
      <PaperCardHeader
        title={navItem.label}
        right={<PaperCardBadge>{t('settings.derivedOnly')}</PaperCardBadge>}
      />
      <PaperCardBody>
        <div className="flex flex-col gap-3">
          <StatusCallout
            tone="info"
            title={t('settings.derivedStateBoundaryTitle')}
            body={t('settings.derivedStateBoundaryBody')}
            actions={
              <div className="flex flex-wrap items-center gap-2">
                <button
                  className="border-border-default text-ink-muted hover:border-ink-muted hover:bg-hover rounded-paper inline-flex items-center border px-3 py-1.5 font-sans text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-60"
                  type="button"
                  disabled={Boolean(state.action)}
                  onClick={() => {
                    void state.onRebuildDerivedState()
                  }}
                >
                  {t('settings.rebuildDerivedState')}
                </button>
                <button
                  className="border-danger text-danger hover:bg-danger-soft rounded-paper inline-flex items-center border px-3 py-1.5 font-sans text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-60"
                  type="button"
                  disabled={Boolean(state.action)}
                  onClick={() => {
                    void state.onClearDerivedState()
                  }}
                >
                  {t('settings.clearDerivedState')}
                </button>
              </div>
            }
          />
          <StatusCallout
            tone="info"
            title={settingsNs('firstPartyRuntimeTitle')}
            body={settingsNs('firstPartyRuntimeBody')}
          />

          <DerivedSearchRulesReview
            action={state.action}
            searchEngineRuleDraft={state.searchEngineRuleDraft}
            searchEngineRuleDraftValid={state.searchEngineRuleDraftValid}
            searchEngineRuleError={state.searchEngineRuleError}
            searchEngineRules={state.searchEngineRules}
            searchEngineRulesLoading={state.searchEngineRulesLoading}
            onCancelSearchEngineRuleEdit={state.onCancelSearchEngineRuleEdit}
            onDeleteSearchEngineRule={state.onDeleteSearchEngineRule}
            onEditSearchEngineRule={state.onEditSearchEngineRule}
            onSaveSearchEngineRule={state.onSaveSearchEngineRule}
            onSearchEngineRuleDraftChange={state.onSearchEngineRuleDraftChange}
            onStartSearchEngineRule={state.onStartSearchEngineRule}
          />

          <DerivedRuntimeReview
            action={state.action}
            clearReport={state.clearReport}
            dashboardRecentRun={state.dashboardRecentRun}
            intelligenceRuntime={state.intelligenceRuntime}
            intelligenceRuntimeError={state.intelligenceRuntimeError}
            rebuildQueueReport={state.rebuildQueueReport}
            snapshot={snapshot}
            onCancelRuntimeJob={state.onCancelRuntimeJob}
            onDeterministicModuleToggle={state.onDeterministicModuleToggle}
            onEnrichmentPluginToggle={state.onEnrichmentPluginToggle}
            onRetryRuntimeJob={state.onRetryRuntimeJob}
          />
        </div>
      </PaperCardBody>
    </PaperCard>
  )
}
