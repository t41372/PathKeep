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
import {
  PaperCard,
  PaperCardBody,
  PaperCardHeader,
} from '@/components/cards'
import { StatusCallout } from '../../components/primitives/status-callout'
import { formatBytes } from '../../lib/format'
import type { RetentionPreview, RetentionPruneResult } from '../../lib/types'
import { useI18n } from '../../lib/i18n'
import { cn } from '../../lib/cn'
import type { SettingsSectionNavItem } from './section-nav-items'

const BUTTON_SECONDARY =
  'border-border-default text-ink-muted hover:border-ink-muted hover:bg-hover rounded-paper inline-flex items-center border px-3 py-1.5 font-sans text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-60'
const BUTTON_DANGER =
  'border-danger text-danger hover:bg-danger-soft rounded-paper inline-flex items-center border px-3 py-1.5 font-sans text-[12px] transition-colors disabled:cursor-not-allowed disabled:opacity-60'

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
 * Paper aesthetic: PaperCard wrapper with mono "selected" chip in the header
 * right slot; retention bar uses bg-accent-soft / bg-accent fills; bucket
 * checkboxes are native inputs in a flex column; warnings get a paper
 * StatusCallout. Destructive button uses BUTTON_DANGER.
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
    <PaperCard testId={navItem.id}>
      <span id={navItem.id} aria-hidden />
      <PaperCardHeader
        title={navItem.label}
        right={
          <span className="text-ink-faint font-mono text-[10.5px]">
            {t('settings.retentionSelected', {
              size: formatBytes(selectedBytes, language),
            })}
          </span>
        }
      />
      <PaperCardBody>
        <p className="text-ink-muted m-0 mb-4 font-serif text-[13.5px] leading-[1.55] italic">
          {t('settings.retentionDescription')}
        </p>

        {needsUnlock ? (
          <div className="mb-3">
            <StatusCallout
              tone="warning"
              title={t('settings.retentionUnlockTitle')}
              body={t('settings.retentionUnlockBody')}
              actions={
                <Link className={BUTTON_SECONDARY} to="/security">
                  {t('navigation.securityLabel')}
                </Link>
              }
            />
          </div>
        ) : null}

        {preview ? (
          <>
            <div className="bg-border-light rounded-paper mb-3 flex h-2 w-full overflow-hidden">
              {preview.buckets.map((bucket) => {
                const totalBytes = preview.buckets.reduce(
                  (sum, currentBucket) => sum + currentBucket.bytes,
                  0,
                )
                const percentage =
                  totalBytes > 0 ? (bucket.bytes / totalBytes) * 100 : 0
                return (
                  <div
                    className={cn(
                      'h-full transition-colors',
                      selection[bucket.id] ? 'bg-accent' : 'bg-accent-soft',
                    )}
                    key={bucket.id}
                    style={{ width: `${Math.max(percentage, 2)}%` }}
                    title={`${bucket.id}: ${formatBytes(bucket.bytes, language)}`}
                  />
                )
              })}
            </div>
            <div className="flex flex-col gap-2">
              {preview.buckets.map((bucket) => (
                <label
                  className="text-ink flex items-center gap-2 font-sans text-[12px]"
                  key={bucket.id}
                >
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
                    <span className="text-ink-faint ml-2 font-mono text-[10.5px]">
                      {formatBytes(bucket.bytes, language)} ·{' '}
                      {bucket.itemCount.toLocaleString(language)}{' '}
                      {t('settings.retentionItems')}
                    </span>
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

        {preview?.warnings.length ? (
          <div className="mt-3 flex flex-col gap-2">
            {preview.warnings.map((warning) => {
              const localizedWarning =
                warning ===
                'Pruning snapshots removes saved restore checkpoints from future Audit review. Manifest and run summaries stay in place.'
                  ? t('settings.retentionSnapshotPruneWarning')
                  : warning ===
                      'Export pruning only removes local files under the PathKeep data directory. Remote objects are unchanged.'
                    ? t('settings.retentionExportPruneWarning')
                    : warning

              return (
                <StatusCallout
                  key={warning}
                  tone="warning"
                  title={t('common.warning')}
                  body={localizedWarning}
                />
              )
            })}
          </div>
        ) : null}

        <div className="mt-4 flex flex-wrap items-center gap-2">
          <button
            className={BUTTON_SECONDARY}
            type="button"
            onClick={() => {
              void onRefresh()
            }}
          >
            {t('settings.retentionRefresh')}
          </button>
          <button
            className={BUTTON_DANGER}
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
          <div className="border-border-light mt-4 flex flex-col gap-2 border-t pt-3">
            <p className="text-ink-muted m-0 font-mono text-[11.5px]">
              {t('settings.retentionDeletedBytes', {
                size: formatBytes(result.deletedBytes, language),
              })}
            </p>
            <p className="text-ink-muted m-0 font-mono text-[11.5px]">
              {t('settings.retentionDeletedFiles', {
                count: result.deletedFiles,
              })}
            </p>
            {result.runId ? (
              <div>
                <Link
                  className={BUTTON_SECONDARY}
                  to={`/audit?run=${result.runId}`}
                >
                  {t('settings.retentionOpenAudit')}
                </Link>
              </div>
            ) : null}
          </div>
        ) : null}

        {error ? (
          <p
            className="text-danger m-0 mt-3 font-mono text-[11px]"
            role="alert"
          >
            {error}
          </p>
        ) : null}
      </PaperCardBody>
    </PaperCard>
  )
}
