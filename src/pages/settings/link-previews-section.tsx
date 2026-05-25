/**
 * Settings → Link previews section.
 *
 * Surfaces the user-facing knobs for the og:image cache that the v0.3
 * paper Browse card mode populates lazily:
 * - Fetch toggle (mirrors AppConfig.ogImage.fetchEnabled — default on).
 * - Per-host blocklist textarea (newline-separated). Persists via
 *   `AppConfig.ogImage.blockedHosts`.
 * - Eviction-mode segmented control (Off / TimeTtl / SizeCap / LRU)
 *   with the per-mode numeric input (max age days / max bytes MB).
 * - Live cache footprint (rows · blobs · bytes).
 * - "Run cleanup now" + "Clear all link previews" actions.
 *
 * Persistence:
 * - Every change writes through `saveConfig` from the shell-data context
 *   so all routes see the new value immediately.
 * - Stats are reloaded after cleanup / clear so the user sees the
 *   storage drop without needing to refresh manually.
 *
 * Numeric inputs:
 * - max_age_days: 1-3650 (about ten years). Clamped on commit.
 * - max_bytes: presented in MB to the user (1-65536 MB) and converted
 *   to bytes for the backend.
 */

import { useCallback, useEffect, useMemo, useState } from 'react'
import { useI18n } from '@/lib/i18n'
import { useShellData } from '@/app/shell-data-context'
import { backend } from '@/lib/backend-client'
import { formatBytes } from '@/lib/format'
import { PaperCard, PaperCardBody, PaperCardHeader } from '@/components/cards'
import type {
  OgImageCleanupMode,
  OgImageSettings,
  OgImageStorageStats,
} from '@/lib/types'
import { cn } from '@/lib/cn'
import { Field, SegmentedControl, Toggle } from './paper-form-primitives'
import { clampNumber, parseBlocklist } from './link-previews-helpers'

export interface LinkPreviewsSectionProps {
  anchorId?: string
}

type CleanupModeId = OgImageCleanupMode['mode']
type FetchModeId = OgImageSettings['fetchMode']

const DEFAULT_OG_IMAGE_SETTINGS: OgImageSettings = {
  fetchEnabled: true,
  fetchMode: 'background',
  dailyRefetchBudget: 50,
  newVisitPrefetchBudget: 100,
  blockedHosts: [],
  cleanup: { mode: 'off' },
}

const REFETCH_BUDGET_MIN = 0
const REFETCH_BUDGET_MAX = 5000
const PREFETCH_BUDGET_MIN = 0
const PREFETCH_BUDGET_MAX = 5000
const REBUILD_DEFAULT_BUDGET = 500
const REBUILD_MAX_BUDGET = 5000

const MAX_AGE_DAYS_DEFAULT = 60
const MAX_AGE_DAYS_MIN = 1
const MAX_AGE_DAYS_MAX = 3650

const MAX_BYTES_DEFAULT_MB = 200
const MAX_BYTES_MIN_MB = 1
const MAX_BYTES_MAX_MB = 65_536

const BYTES_PER_MB = 1_024 * 1_024

function modeId(mode: OgImageCleanupMode): CleanupModeId {
  return mode.mode
}

function isTimeTtl(
  mode: OgImageCleanupMode,
): mode is { mode: 'timeTtl'; maxAgeDays: number } {
  return mode.mode === 'timeTtl'
}

function isSizeCapOrLru(
  mode: OgImageCleanupMode,
): mode is { mode: 'sizeCap' | 'lru'; maxBytes: number } {
  return mode.mode === 'sizeCap' || mode.mode === 'lru'
}

export function LinkPreviewsSection({
  anchorId = 'link-previews',
}: LinkPreviewsSectionProps) {
  const { language, t } = useI18n()
  const { snapshot, saveConfig } = useShellData()
  const [stats, setStats] = useState<OgImageStorageStats | null>(null)
  const [pendingAction, setPendingAction] = useState<
    'cleanup' | 'clear' | 'rebuild' | null
  >(null)
  const [summary, setSummary] = useState<string | null>(null)

  const settings = snapshot?.config.ogImage ?? DEFAULT_OG_IMAGE_SETTINGS
  const fetchEnabled = settings.fetchEnabled
  const cleanup = settings.cleanup
  const selectedModeId = modeId(cleanup)

  const [blocklistDraft, setBlocklistDraft] = useState<string>(
    settings.blockedHosts.join('\n'),
  )
  const blocklistChanged = useMemo(() => {
    const parsed = parseBlocklist(blocklistDraft)
    return (
      parsed.length !== settings.blockedHosts.length ||
      parsed.some((host, i) => host !== settings.blockedHosts[i])
    )
  }, [blocklistDraft, settings.blockedHosts])

  const persistSettings = useCallback(
    async (next: OgImageSettings) => {
      if (!snapshot) return
      await saveConfig({
        ...snapshot.config,
        ogImage: next,
      })
    },
    [snapshot, saveConfig],
  )

  const refreshStats = useCallback(async () => {
    try {
      const next = await backend.getOgImageStorageStats()
      setStats(next)
    } catch {
      // Stats are diagnostic; a transient failure isn't worth surfacing
      // as a user-visible error toast. The next refresh attempt covers it.
    }
  }, [])

  useEffect(() => {
    void refreshStats()
  }, [refreshStats])

  const onToggleFetch = (next: boolean) =>
    persistSettings({ ...settings, fetchEnabled: next })

  const onSelectFetchMode = async (next: FetchModeId) => {
    if (next === settings.fetchMode) return
    await persistSettings({ ...settings, fetchMode: next })
  }

  const onChangeRefetchBudget = async (raw: string) => {
    const next = clampNumber(
      raw,
      REFETCH_BUDGET_MIN,
      REFETCH_BUDGET_MAX,
      settings.dailyRefetchBudget,
    )
    if (next === settings.dailyRefetchBudget) return
    await persistSettings({ ...settings, dailyRefetchBudget: next })
  }

  const onChangePrefetchBudget = async (raw: string) => {
    const next = clampNumber(
      raw,
      PREFETCH_BUDGET_MIN,
      PREFETCH_BUDGET_MAX,
      settings.newVisitPrefetchBudget,
    )
    if (next === settings.newVisitPrefetchBudget) return
    await persistSettings({ ...settings, newVisitPrefetchBudget: next })
  }

  const onRebuildNow = async () => {
    setPendingAction('rebuild')
    try {
      const [enqueued, succeeded] = await backend.prefetchOgImages(
        REBUILD_DEFAULT_BUDGET,
      )
      setSummary(
        t('settings.linkPreviewsRebuildSummary', {
          enqueued: String(enqueued),
          succeeded: String(succeeded),
        }),
      )
      await refreshStats()
    } catch (error) {
      // Swallow the worker error and surface a short summary instead of
      // letting an unhandled rejection escape the click handler. The
      // worker still persists negative-cache rows for whatever it could
      // process, and the user can retry by clicking the button again
      // (the finally re-enables it).
      setSummary(
        error instanceof Error
          ? `${t('settings.linkPreviewsRebuildSummary', {
              enqueued: '0',
              succeeded: '0',
            })} (${error.message})`
          : t('settings.linkPreviewsRebuildSummary', {
              enqueued: '0',
              succeeded: '0',
            }),
      )
    } finally {
      setPendingAction(null)
    }
  }

  const onBlocklistSave = async () => {
    await persistSettings({
      ...settings,
      blockedHosts: parseBlocklist(blocklistDraft),
    })
  }

  const onSelectMode = async (id: CleanupModeId) => {
    if (id === selectedModeId) return
    let nextMode: OgImageCleanupMode
    if (id === 'off') {
      nextMode = { mode: 'off' }
    } else if (id === 'timeTtl') {
      const maxAgeDays = isTimeTtl(cleanup)
        ? cleanup.maxAgeDays
        : MAX_AGE_DAYS_DEFAULT
      nextMode = { mode: 'timeTtl', maxAgeDays }
    } else {
      const maxBytes = isSizeCapOrLru(cleanup)
        ? cleanup.maxBytes
        : MAX_BYTES_DEFAULT_MB * BYTES_PER_MB
      nextMode = { mode: id, maxBytes }
    }
    await persistSettings({ ...settings, cleanup: nextMode })
  }

  const onChangeMaxAgeDays = async (raw: string) => {
    if (!isTimeTtl(cleanup)) return
    const next = clampNumber(
      raw,
      MAX_AGE_DAYS_MIN,
      MAX_AGE_DAYS_MAX,
      cleanup.maxAgeDays,
    )
    await persistSettings({
      ...settings,
      cleanup: { mode: 'timeTtl', maxAgeDays: next },
    })
  }

  const onChangeMaxBytesMb = async (raw: string) => {
    if (!isSizeCapOrLru(cleanup)) return
    const nextMb = clampNumber(
      raw,
      MAX_BYTES_MIN_MB,
      MAX_BYTES_MAX_MB,
      Math.round(cleanup.maxBytes / BYTES_PER_MB),
    )
    await persistSettings({
      ...settings,
      cleanup: { mode: cleanup.mode, maxBytes: nextMb * BYTES_PER_MB },
    })
  }

  const onRunCleanup = async () => {
    setPendingAction('cleanup')
    try {
      const report = await backend.runOgImageCleanup()
      setSummary(
        t('settings.linkPreviewsCleanupSummary', {
          rows: String(report.deletedRows),
          blobs: String(report.deletedBlobs),
          bytes: formatBytes(report.reclaimedBytes, language),
        }),
      )
      await refreshStats()
    } finally {
      setPendingAction(null)
    }
  }

  const onClearAll = async () => {
    if (!window.confirm(t('settings.linkPreviewsClearConfirm'))) return
    setPendingAction('clear')
    try {
      const report = await backend.clearOgImageCache()
      setSummary(
        t('settings.linkPreviewsCleanupSummary', {
          rows: String(report.deletedRows),
          blobs: String(report.deletedBlobs),
          bytes: formatBytes(report.reclaimedBytes, language),
        }),
      )
      await refreshStats()
    } finally {
      setPendingAction(null)
    }
  }

  const hasRows = (stats?.rowCount ?? 0) > 0

  const modeOptions: Array<{
    id: CleanupModeId
    label: string
    hint?: string
  }> = [
    {
      id: 'off',
      label: t('settings.linkPreviewsCleanupModeOff'),
      hint: t('settings.linkPreviewsCleanupModeOffHint'),
    },
    {
      id: 'timeTtl',
      label: t('settings.linkPreviewsCleanupModeTimeTtl'),
      hint: t('settings.linkPreviewsCleanupModeTimeTtlHint'),
    },
    {
      id: 'sizeCap',
      label: t('settings.linkPreviewsCleanupModeSizeCap'),
      hint: t('settings.linkPreviewsCleanupModeSizeCapHint'),
    },
    {
      id: 'lru',
      label: t('settings.linkPreviewsCleanupModeLru'),
      hint: t('settings.linkPreviewsCleanupModeLruHint'),
    },
  ]

  return (
    <PaperCard testId="settings-link-previews-section" id={anchorId}>
      <PaperCardHeader title={t('settings.linkPreviewsTitle')} />
      <PaperCardBody>
        <p className="text-ink-muted m-0 mb-4 font-serif text-[13.5px] leading-[1.55] italic">
          {t('settings.linkPreviewsIntro')}
        </p>

        <Field
          label={t('settings.linkPreviewsFetchToggleLabel')}
          help={t('settings.linkPreviewsFetchToggleHint')}
        >
          <Toggle
            value={fetchEnabled}
            onChange={onToggleFetch}
            onLabel={t('settings.linkPreviewsFetchOn')}
            offLabel={t('settings.linkPreviewsFetchOff')}
            testId="link-previews-fetch-toggle"
          />
        </Field>

        <Field
          label={t('settings.linkPreviewsFetchModeLabel')}
          help={t('settings.linkPreviewsFetchModeHint')}
        >
          <SegmentedControl<FetchModeId>
            options={[
              {
                id: 'off',
                label: t('settings.linkPreviewsFetchModeOff'),
                hint: t('settings.linkPreviewsFetchModeOffHint'),
              },
              {
                id: 'on_demand',
                label: t('settings.linkPreviewsFetchModeOnDemand'),
                hint: t('settings.linkPreviewsFetchModeOnDemandHint'),
              },
              {
                id: 'background',
                label: t('settings.linkPreviewsFetchModeBackground'),
                hint: t('settings.linkPreviewsFetchModeBackgroundHint'),
              },
            ]}
            value={settings.fetchMode}
            onChange={(id) => void onSelectFetchMode(id)}
            stacked
            disabled={!fetchEnabled}
            testId="link-previews-fetch-mode"
          />
        </Field>

        <Field
          label={t('settings.linkPreviewsBudgetsLabel')}
          help={t('settings.linkPreviewsBudgetsHint')}
        >
          <div className="flex flex-wrap items-center gap-4 font-mono text-[11px] text-ink-muted">
            <label className="flex items-center gap-2">
              <span>{t('settings.linkPreviewsDailyRefetchBudgetLabel')}</span>
              <input
                type="number"
                inputMode="numeric"
                min={REFETCH_BUDGET_MIN}
                max={REFETCH_BUDGET_MAX}
                step={1}
                value={settings.dailyRefetchBudget}
                disabled={!fetchEnabled}
                onChange={(event) =>
                  void onChangeRefetchBudget(event.target.value)
                }
                data-testid="link-previews-daily-refetch-budget"
                className="border-border-default rounded-paper bg-paper w-24 border px-2 py-1 text-right disabled:opacity-60"
              />
            </label>
            <label className="flex items-center gap-2">
              <span>{t('settings.linkPreviewsPrefetchBudgetLabel')}</span>
              <input
                type="number"
                inputMode="numeric"
                min={PREFETCH_BUDGET_MIN}
                max={PREFETCH_BUDGET_MAX}
                step={1}
                value={settings.newVisitPrefetchBudget}
                disabled={
                  !fetchEnabled || settings.fetchMode !== 'background'
                }
                onChange={(event) =>
                  void onChangePrefetchBudget(event.target.value)
                }
                data-testid="link-previews-prefetch-budget"
                className="border-border-default rounded-paper bg-paper w-24 border px-2 py-1 text-right disabled:opacity-60"
              />
            </label>
          </div>
        </Field>

        <Field
          label={t('settings.linkPreviewsBlocklistLabel')}
          help={t('settings.linkPreviewsBlocklistHint')}
        >
          <textarea
            value={blocklistDraft}
            onChange={(event) => setBlocklistDraft(event.target.value)}
            rows={4}
            placeholder={t('settings.linkPreviewsBlocklistPlaceholder')}
            data-testid="link-previews-blocklist-input"
            className={cn(
              'border-border-default rounded-paper bg-paper w-full resize-y border px-3 py-2 font-mono text-[11.5px] text-ink',
              'focus:border-accent focus:outline-none',
            )}
          />
          {blocklistChanged ? (
            <div className="mt-2 flex items-center gap-2">
              <button
                type="button"
                onClick={() => void onBlocklistSave()}
                data-testid="link-previews-blocklist-save"
                className="border-accent text-accent-text hover:bg-accent-soft rounded-paper border px-3 py-1 font-sans text-[12px]"
              >
                {t('settings.linkPreviewsBlocklistSave')}
              </button>
              <button
                type="button"
                onClick={() =>
                  setBlocklistDraft(settings.blockedHosts.join('\n'))
                }
                data-testid="link-previews-blocklist-reset"
                className="border-border-default text-ink-muted hover:bg-hover rounded-paper border px-3 py-1 font-sans text-[12px]"
              >
                {t('settings.linkPreviewsBlocklistReset')}
              </button>
            </div>
          ) : null}
        </Field>

        <Field
          label={t('settings.linkPreviewsCleanupModeLabel')}
          help={t('settings.linkPreviewsCleanupModeHint')}
        >
          <SegmentedControl
            options={modeOptions}
            value={selectedModeId}
            onChange={(id) => void onSelectMode(id)}
            stacked
            testId="link-previews-cleanup-mode"
          />
          {isTimeTtl(cleanup) ? (
            <label className="mt-3 flex items-center gap-2 font-mono text-[11px] text-ink-muted">
              <span>{t('settings.linkPreviewsMaxAgeDaysLabel')}</span>
              <input
                type="number"
                inputMode="numeric"
                min={MAX_AGE_DAYS_MIN}
                max={MAX_AGE_DAYS_MAX}
                step={1}
                value={cleanup.maxAgeDays}
                onChange={(event) =>
                  void onChangeMaxAgeDays(event.target.value)
                }
                data-testid="link-previews-max-age-days"
                className="border-border-default rounded-paper bg-paper w-24 border px-2 py-1 text-right"
              />
              <span>{t('settings.linkPreviewsMaxAgeDaysUnit')}</span>
            </label>
          ) : null}
          {isSizeCapOrLru(cleanup) ? (
            <label className="mt-3 flex items-center gap-2 font-mono text-[11px] text-ink-muted">
              <span>{t('settings.linkPreviewsMaxBytesLabel')}</span>
              <input
                type="number"
                inputMode="numeric"
                min={MAX_BYTES_MIN_MB}
                max={MAX_BYTES_MAX_MB}
                step={1}
                value={Math.max(
                  MAX_BYTES_MIN_MB,
                  Math.round(cleanup.maxBytes / BYTES_PER_MB),
                )}
                onChange={(event) =>
                  void onChangeMaxBytesMb(event.target.value)
                }
                data-testid="link-previews-max-bytes-mb"
                className="border-border-default rounded-paper bg-paper w-24 border px-2 py-1 text-right"
              />
              <span>{t('settings.linkPreviewsMaxBytesUnit')}</span>
            </label>
          ) : null}
        </Field>

        <Field label={t('settings.linkPreviewsStatsLabel')}>
          <p
            className="text-ink-muted m-0 font-mono text-[11.5px]"
            data-testid="link-previews-stats"
          >
            {hasRows && stats
              ? t('settings.linkPreviewsStatsRows', {
                  rows: String(stats.rowCount),
                  blobs: String(stats.blobCount),
                  bytes: formatBytes(stats.totalBytes, language),
                })
              : t('settings.linkPreviewsStatsEmpty')}
          </p>
        </Field>

        <Field label={t('settings.linkPreviewsCleanupLabel')}>
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              className={cn(
                'border-accent text-accent-text bg-paper rounded-paper border px-3 py-1.5 font-sans text-[12px] transition-colors',
                pendingAction || !fetchEnabled
                  ? 'opacity-60'
                  : 'hover:bg-accent-soft',
              )}
              disabled={pendingAction !== null || !fetchEnabled}
              onClick={() => void onRebuildNow()}
              data-testid="link-previews-rebuild-now"
              title={t('settings.linkPreviewsRebuildHint', {
                budget: String(REBUILD_DEFAULT_BUDGET),
                cap: String(REBUILD_MAX_BUDGET),
              })}
            >
              {t('settings.linkPreviewsRebuildAction', {
                budget: String(REBUILD_DEFAULT_BUDGET),
              })}
            </button>
            <button
              type="button"
              className={cn(
                'border-border-default rounded-paper text-ink border px-3 py-1.5 font-sans text-[12px] transition-colors',
                pendingAction
                  ? 'opacity-60'
                  : 'hover:border-ink-muted hover:bg-hover',
              )}
              disabled={pendingAction !== null}
              onClick={() => void onRunCleanup()}
              data-testid="link-previews-run-cleanup"
            >
              {t('settings.linkPreviewsRunCleanupAction')}
            </button>
            <button
              type="button"
              className={cn(
                'border-border-default text-ink-muted rounded-paper border px-3 py-1.5 font-sans text-[12px] transition-colors',
                pendingAction
                  ? 'opacity-60'
                  : 'hover:border-warning hover:text-warning hover:bg-hover',
              )}
              disabled={pendingAction !== null}
              onClick={() => void onClearAll()}
              data-testid="link-previews-clear-all"
            >
              {t('settings.linkPreviewsClearAllAction')}
            </button>
            {summary ? (
              <span
                className="text-ink-faint font-mono text-[10.5px]"
                data-testid="link-previews-summary"
              >
                {summary}
              </span>
            ) : null}
          </div>
        </Field>
      </PaperCardBody>
    </PaperCard>
  )
}
