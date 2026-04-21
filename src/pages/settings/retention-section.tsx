/**
 * @file retention-section.tsx
 * @description Renders the retention preview and prune review surface from route-owned state.
 * @module pages/settings
 *
 * ## 職責
 * - 顯示 retention bucket 預覽、warnings、selection summary 與 prune 結果。
 * - 把 refresh / prune / bucket selection 行為交回 route hook。
 * - 保持 retention panel 的 PME honesty，不在 section 內偷偷觸發 destructive work。
 *
 * ## 不負責
 * - 不直接載入 retention preview。
 * - 不保存 selection 到全域狀態。
 * - 不自行決定 archive unlock 或 audit navigation 規則。
 *
 * ## 依賴關係
 * - 依賴 route hook 提供 preview、selection、result 與 mutation handlers。
 * - 依賴 `useI18n()`、`formatBytes()` 與 `Link` 呈現 localized review copy。
 *
 * ## 性能備注
 * - 只消費既有 preview payload，不自行觸發額外查詢，避免 retention panel 成為第二個 background loader。
 */

import { Link } from 'react-router-dom'
import { StatusCallout } from '../../components/primitives/status-callout'
import { Glyph } from '../../components/ui'
import { formatBytes } from '../../lib/format'
import type { RetentionPreview, RetentionPruneResult } from '../../lib/types'
import { useI18n } from '../../lib/i18n'
import type { SettingsSectionNavItem } from './section-nav-items'

/**
 * Defines the route-owned retention review state consumed by this section.
 */
export interface RetentionSectionState {
  action: string | null
  error: string | null
  needsUnlock: boolean
  preview: RetentionPreview | null
  result: RetentionPruneResult | null
  selectedBytes: number
  selection: Record<string, boolean>
  onBucketSelectionChange: (bucketId: string, checked: boolean) => void
  onPrune: () => Promise<void>
  onRefresh: () => Promise<void>
}

/**
 * Groups the stable section anchor descriptor with the retention view-model.
 */
export interface RetentionSectionProps {
  navItem: SettingsSectionNavItem
  state: RetentionSectionState
}

/**
 * Renders the retention preview and prune controls from route-owned state.
 *
 * The section keeps the visual review surface local while destructive work
 * remains centralized in the route hook.
 */
export function RetentionSection({ navItem, state }: RetentionSectionProps) {
  const { language, t } = useI18n()
  const {
    action,
    error,
    needsUnlock,
    preview,
    result,
    selectedBytes,
    selection,
    onBucketSelectionChange,
    onPrune,
    onRefresh,
  } = state
  const selectedBuckets = preview
    ? preview.buckets.filter((bucket) => selection[bucket.id])
    : []

  return (
    <div className="panel panel--critical" id={navItem.id}>
      <div className="panel-header">
        <span className="panel-title">
          <Glyph icon={navItem.icon} filled />
          <span>{navItem.label}</span>
        </span>
        <span className="panel-action">
          {t('settings.retentionSelected', {
            size: formatBytes(selectedBytes, language),
          })}
        </span>
      </div>
      <div className="panel-body">
        <p className="dashboard-next-action">
          {t('settings.retentionDescription')}
        </p>

        {needsUnlock ? (
          <StatusCallout
            tone="warning"
            title={t('settings.retentionUnlockTitle')}
            body={t('settings.retentionUnlockBody')}
            actions={
              <Link className="btn-secondary" to="/security">
                {t('navigation.securityLabel')}
              </Link>
            }
          />
        ) : null}

        {preview ? (
          <>
            <div className="retention-bar">
              {preview.buckets.map((bucket) => {
                const totalBytes = preview.buckets.reduce(
                  (sum, currentBucket) => sum + currentBucket.bytes,
                  0,
                )
                const percentage =
                  totalBytes > 0 ? (bucket.bytes / totalBytes) * 100 : 0
                return (
                  <div
                    className={`retention-bar__segment ${selection[bucket.id] ? 'retention-bar__segment--selected' : ''}`}
                    key={bucket.id}
                    style={{ width: `${Math.max(percentage, 2)}%` }}
                    title={`${bucket.id}: ${formatBytes(bucket.bytes, language)}`}
                  />
                )
              })}
            </div>
            <div className="settings-field-grid">
              {preview.buckets.map((bucket) => (
                <label className="checkbox-row" key={bucket.id}>
                  <input
                    checked={Boolean(selection[bucket.id])}
                    type="checkbox"
                    onChange={(event) => {
                      onBucketSelectionChange(bucket.id, event.target.checked)
                    }}
                  />
                  <span>
                    {bucket.id === 'snapshots'
                      ? t('settings.retentionSnapshots')
                      : bucket.id === 'exports'
                        ? t('settings.retentionExports')
                        : bucket.id === 'staging'
                          ? t('settings.retentionStaging')
                          : t('settings.retentionQuarantine')}
                    {` · ${formatBytes(bucket.bytes, language)} · ${bucket.itemCount.toLocaleString(language)} ${t('settings.retentionItems')}`}
                  </span>
                </label>
              ))}
            </div>
          </>
        ) : (
          <StatusCallout
            tone="info"
            title={t('settings.retentionLoadingTitle')}
            body={t('common.loading')}
          />
        )}

        {preview?.warnings.map((warning) => (
          <div className="warning-box" key={warning}>
            <div className="warning-icon">
              <Glyph icon="warning" filled />
            </div>
            <div className="warning-text">{warning}</div>
          </div>
        ))}

        <div className="wizard-actions">
          <button
            className="btn-secondary"
            type="button"
            onClick={() => {
              void onRefresh()
            }}
          >
            {t('settings.retentionRefresh')}
          </button>
          <button
            className="btn-danger"
            type="button"
            disabled={
              needsUnlock || Boolean(action) || selectedBuckets.length === 0
            }
            onClick={() => {
              void onPrune()
            }}
          >
            {action ?? t('settings.retentionExecute')}
          </button>
        </div>

        {result ? (
          <div
            className="inline-note-list"
            style={{ marginTop: 'var(--space-3)' }}
          >
            <div className="result-row">
              <p>
                {t('settings.retentionDeletedBytes', {
                  size: formatBytes(result.deletedBytes, language),
                })}
              </p>
            </div>
            <div className="result-row">
              <p>
                {t('settings.retentionDeletedFiles', {
                  count: result.deletedFiles,
                })}
              </p>
            </div>
            {result.runId ? (
              <div className="result-row">
                <Link
                  className="btn-secondary"
                  to={`/audit?run=${result.runId}`}
                >
                  {t('settings.retentionOpenAudit')}
                </Link>
              </div>
            ) : null}
          </div>
        ) : null}

        {error ? (
          <p className="inline-error" role="alert">
            {error}
          </p>
        ) : null}
      </div>
    </div>
  )
}
