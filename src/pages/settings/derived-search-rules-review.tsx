/**
 * @file derived-search-rules-review.tsx
 * @description Renders the search-engine rules review and editor surface used inside Settings derived-state.
 * @module pages/settings
 *
 * ## 職責
 * - 顯示 built-in/custom search-engine rules 與 editor。
 * - 把 add/edit/save/delete/cancel 行為交回 route-owned handlers。
 * - 保持 search rule review surface 和 derived rebuild truth 對齊。
 *
 * ## 不負責
 * - 不載入 rules 或執行 rebuild。
 * - 不管理 runtime queue。
 * - 不定義 search-rule persistence contract。
 *
 * ## 依賴關係
 * - 依賴 derived-state route hook 提供 rule state 與 handlers。
 *
 * ## 性能備注
 * - 只處理 route 已提供的 rule arrays，不做額外查詢。
 */

import { useMemo } from 'react'
import { StatusCallout } from '../../components/primitives/status-callout'
import { useI18n } from '../../lib/i18n'
import type {
  SearchEngineRule,
  SearchEngineRuleInput,
} from '../../lib/core-intelligence/types'

/**
 * Props for the extracted search-rules review surface.
 */
export interface DerivedSearchRulesReviewProps {
  action: string | null
  searchEngineRuleDraft: SearchEngineRuleInput | null
  searchEngineRuleDraftValid: boolean
  searchEngineRuleError: string | null
  searchEngineRules: SearchEngineRule[]
  searchEngineRulesLoading: boolean
  onCancelSearchEngineRuleEdit: () => void
  onDeleteSearchEngineRule: (ruleId: string) => Promise<void>
  onEditSearchEngineRule: (rule: SearchEngineRule) => void
  onSaveSearchEngineRule: () => Promise<void>
  onSearchEngineRuleDraftChange: (patch: Partial<SearchEngineRuleInput>) => void
  onStartSearchEngineRule: () => void
}

/**
 * Renders the search-engine rule browser and editor from route-owned state.
 */
export function DerivedSearchRulesReview({
  action,
  searchEngineRuleDraft,
  searchEngineRuleDraftValid,
  searchEngineRuleError,
  searchEngineRules,
  searchEngineRulesLoading,
  onCancelSearchEngineRuleEdit,
  onDeleteSearchEngineRule,
  onEditSearchEngineRule,
  onSaveSearchEngineRule,
  onSearchEngineRuleDraftChange,
  onStartSearchEngineRule,
}: DerivedSearchRulesReviewProps) {
  const { t, ns } = useI18n()
  const commonNs = ns('common')
  const settingsNs = ns('settings')
  const builtinSearchEngineRules = useMemo(
    () => searchEngineRules.filter((rule) => rule.builtIn),
    [searchEngineRules],
  )
  const customSearchEngineRules = useMemo(
    () => searchEngineRules.filter((rule) => !rule.builtIn),
    [searchEngineRules],
  )

  return (
    <>
      <StatusCallout
        tone={searchEngineRuleError ? 'warning' : 'info'}
        title={settingsNs('searchRulesTitle')}
        body={
          searchEngineRuleError ??
          (searchEngineRulesLoading
            ? commonNs('loading')
            : settingsNs('searchRulesBody'))
        }
        actions={
          <div className="settings-action-row">
            <button
              className="btn-secondary"
              type="button"
              disabled={Boolean(action) || searchEngineRulesLoading}
              onClick={onStartSearchEngineRule}
            >
              {settingsNs('searchRulesAdd')}
            </button>
          </div>
        }
      />
      <div className="settings-result-list">
        <div className="result-row">
          <div className="result-row__header">
            <strong>{settingsNs('searchRulesBuiltin')}</strong>
            <span className="mono">{settingsNs('searchRulesReadOnly')}</span>
          </div>
          <p>{settingsNs('searchRulesBuiltinBody')}</p>
          {builtinSearchEngineRules.map((rule) => (
            <div className="config-row" key={rule.ruleId}>
              <span className="config-label">{rule.displayName}</span>
              <span className="config-value mono">
                {rule.hostPattern}
                {rule.pathPrefix ? rule.pathPrefix : ''}
                {' ?'}
                {rule.queryParamKey}
              </span>
            </div>
          ))}
        </div>
        <div className="result-row">
          <div className="result-row__header">
            <strong>{settingsNs('searchRulesCustom')}</strong>
            <span className="mono">
              {settingsNs('searchRulesCustomCount', {
                count: customSearchEngineRules.length,
              })}
            </span>
          </div>
          <p>{settingsNs('searchRulesCustomBody')}</p>
          {customSearchEngineRules.length ? (
            customSearchEngineRules.map((rule) => (
              <div className="result-row result-row--active" key={rule.ruleId}>
                <div className="result-row__header">
                  <strong>{rule.displayName}</strong>
                  <span className="mono">
                    {rule.engineId} ·{' '}
                    {rule.enabled
                      ? t('settings.enabled')
                      : t('settings.disabled')}
                  </span>
                </div>
                <div className="config-row">
                  <span className="config-label">
                    {settingsNs('searchRulesHostPattern')}
                  </span>
                  <span className="config-value mono">{rule.hostPattern}</span>
                </div>
                <div className="config-row">
                  <span className="config-label">
                    {settingsNs('searchRulesPathPrefix')}
                  </span>
                  <span className="config-value mono">
                    {rule.pathPrefix || commonNs('notAvailable')}
                  </span>
                </div>
                <div className="config-row">
                  <span className="config-label">
                    {settingsNs('searchRulesQueryParam')}
                  </span>
                  <span className="config-value mono">
                    {rule.queryParamKey}
                  </span>
                </div>
                {rule.note ? <p className="mono-support">{rule.note}</p> : null}
                <div className="settings-action-row">
                  <button
                    className="btn-secondary"
                    type="button"
                    disabled={Boolean(action)}
                    onClick={() => onEditSearchEngineRule(rule)}
                  >
                    {settingsNs('searchRulesEdit')}
                  </button>
                  <button
                    className="btn-danger"
                    type="button"
                    disabled={Boolean(action)}
                    onClick={() => {
                      void onDeleteSearchEngineRule(rule.ruleId)
                    }}
                  >
                    {settingsNs('searchRulesDelete')}
                  </button>
                </div>
              </div>
            ))
          ) : (
            <p>{settingsNs('searchRulesCustomEmpty')}</p>
          )}
        </div>
        {searchEngineRuleDraft ? (
          <section
            aria-label={settingsNs('searchRulesEditorTitle')}
            className="result-row result-row--active"
            data-testid="settings-search-rule-editor"
          >
            <div className="result-row__header">
              <strong>{settingsNs('searchRulesEditorTitle')}</strong>
              <span className="mono">
                {searchEngineRuleDraft.ruleId
                  ? settingsNs('searchRulesEditing')
                  : settingsNs('searchRulesNew')}
              </span>
            </div>
            <div className="settings-remote-grid">
              <label className="field-stack">
                <span className="mono-kicker">
                  {settingsNs('searchRulesDisplayName')}
                </span>
                <input
                  type="text"
                  value={searchEngineRuleDraft.displayName}
                  onChange={(event) =>
                    onSearchEngineRuleDraftChange({
                      displayName: event.target.value,
                    })
                  }
                />
              </label>
              <label className="field-stack">
                <span className="mono-kicker">
                  {settingsNs('searchRulesEngineId')}
                </span>
                <input
                  type="text"
                  value={searchEngineRuleDraft.engineId}
                  onChange={(event) =>
                    onSearchEngineRuleDraftChange({
                      engineId: event.target.value,
                    })
                  }
                />
              </label>
              <label className="field-stack">
                <span className="mono-kicker">
                  {settingsNs('searchRulesHostPattern')}
                </span>
                <input
                  type="text"
                  value={searchEngineRuleDraft.hostPattern}
                  onChange={(event) =>
                    onSearchEngineRuleDraftChange({
                      hostPattern: event.target.value,
                    })
                  }
                />
              </label>
              <label className="field-stack">
                <span className="mono-kicker">
                  {settingsNs('searchRulesPathPrefix')}
                </span>
                <input
                  type="text"
                  value={searchEngineRuleDraft.pathPrefix ?? ''}
                  onChange={(event) =>
                    onSearchEngineRuleDraftChange({
                      pathPrefix: event.target.value,
                    })
                  }
                />
              </label>
              <label className="field-stack">
                <span className="mono-kicker">
                  {settingsNs('searchRulesQueryParam')}
                </span>
                <input
                  type="text"
                  value={searchEngineRuleDraft.queryParamKey}
                  onChange={(event) =>
                    onSearchEngineRuleDraftChange({
                      queryParamKey: event.target.value,
                    })
                  }
                />
              </label>
              <label className="field-stack">
                <span className="mono-kicker">
                  {settingsNs('searchRulesExampleUrl')}
                </span>
                <input
                  type="text"
                  value={searchEngineRuleDraft.exampleUrl ?? ''}
                  onChange={(event) =>
                    onSearchEngineRuleDraftChange({
                      exampleUrl: event.target.value,
                    })
                  }
                />
              </label>
              <label className="checkbox-row">
                <input
                  type="checkbox"
                  checked={searchEngineRuleDraft.enabled}
                  onChange={(event) =>
                    onSearchEngineRuleDraftChange({
                      enabled: event.target.checked,
                    })
                  }
                />
                <span>{settingsNs('searchRulesEnabled')}</span>
              </label>
              <label className="field-stack" style={{ gridColumn: '1 / -1' }}>
                <span className="mono-kicker">
                  {settingsNs('searchRulesNote')}
                </span>
                <textarea
                  value={searchEngineRuleDraft.note ?? ''}
                  onChange={(event) =>
                    onSearchEngineRuleDraftChange({ note: event.target.value })
                  }
                />
              </label>
            </div>
            <div className="settings-action-row">
              <button
                className="btn-secondary"
                type="button"
                disabled={Boolean(action) || !searchEngineRuleDraftValid}
                onClick={() => {
                  void onSaveSearchEngineRule()
                }}
              >
                {settingsNs('searchRulesSave')}
              </button>
              <button
                className="btn-secondary"
                type="button"
                disabled={Boolean(action)}
                onClick={onCancelSearchEngineRuleEdit}
              >
                {commonNs('cancel')}
              </button>
            </div>
          </section>
        ) : null}
      </div>
    </>
  )
}
