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
import { describeError } from '@/lib/errors'
import { formatBytes } from '@/lib/format'
import { PaperCard, PaperCardBody, PaperCardHeader } from '@/components/cards'
import type {
  OgImageCleanupMode,
  OgImageCoverageStats,
  OgImageSettings,
  OgImageStorageStats,
} from '@/lib/types'
import { cn } from '@/lib/cn'
import { Field, SegmentedControl, Toggle } from './paper-form-primitives'
import { clampNumber, parseBlocklist } from './link-previews-helpers'
import { SettingsSavedChip } from './settings-saved-feedback'
import { useSavedFeedback } from './use-saved-feedback'

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
  const { visible: savedVisible, flash } = useSavedFeedback()
  const [stats, setStats] = useState<OgImageStorageStats | null>(null)
  const [coverage, setCoverage] = useState<OgImageCoverageStats | null>(null)
  const [coverageFailed, setCoverageFailed] = useState(false)
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
      // `quiet` so this all-auto-save write never throws the blocking full-screen
      // overlay on every toggle/select/blur — the inline "Saved" chip is the only
      // confirmation. The shell still refreshes the snapshot exactly the same.
      await saveConfig(
        {
          ...snapshot.config,
          ogImage: next,
        },
        { quiet: true },
      )
      // Quiet "Saved" confirmation on a landed write — the page is all-auto-save.
      flash()
    },
    [snapshot, saveConfig, flash],
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

  // Coverage is fetched separately: its eligible-page count scans the `urls`
  // table, so a slow query must never delay the cheap storage footprint above.
  const refreshCoverage = useCallback(async () => {
    try {
      const next = await backend.getOgImageCoverageStats()
      setCoverage(next)
      setCoverageFailed(false)
    } catch {
      // Diagnostic only — flag the failure so the row shows "couldn't measure"
      // instead of sitting on the loading text forever. Keep any prior value.
      setCoverageFailed(true)
    }
  }, [])

  useEffect(() => {
    void refreshStats()
    void refreshCoverage()
  }, [refreshStats, refreshCoverage])

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
      void refreshCoverage()
    } catch (error) {
      // Swallow the worker error and surface a short summary instead of
      // letting an unhandled rejection escape the click handler. The
      // worker still persists negative-cache rows for whatever it could
      // process, and the user can retry by clicking the button again
      // (the finally re-enables it).
      setSummary(
        `${t('settings.linkPreviewsRebuildSummary', {
          enqueued: '0',
          succeeded: '0',
        })} (${describeError(error, 'refetch_og_images')})`,
      )
    } finally {
      setPendingAction(null)
    }
  }

  // The per-host blocklist auto-saves on blur (the page is all-auto-save). It is a
  // free-text textarea so it edits a local draft while typing — keeping saveConfig
  // off the keystroke hot path — and only persists when focus leaves and the
  // canonicalized hosts differ from what's saved, so a blur with no edit never
  // fires a redundant write or a misleading "Saved".
  const onCommitBlocklist = () => {
    if (!blocklistChanged) return
    void persistSettings({
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
      void refreshCoverage()
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
      void refreshCoverage()
    } finally {
      setPendingAction(null)
    }
  }

  const hasRows = (stats?.rowCount ?? 0) > 0

  // Honest coverage view: the headline is "% of web pages that have a preview",
  // shown only once it has loaded so we never flash a misleading 0%. The
  // success rate (of pages actually checked) is secondary context, because
  // fetching is opportunistic so the headline is naturally low.
  const coverageView = useMemo(() => {
    if (!coverage || coverage.eligiblePages <= 0) return null
    const pct = (value: number, total: number) =>
      total > 0 ? ((value / total) * 100).toFixed(1) : '0.0'
    const count = (value: number) => value.toLocaleString(language)
    return {
      percent: pct(coverage.pagesWithImage, coverage.eligiblePages),
      withImage: count(coverage.pagesWithImage),
      eligible: count(coverage.eligiblePages),
      hasAttempts: coverage.attemptedPages > 0,
      successRate: pct(coverage.pagesWithImage, coverage.attemptedPages),
      checked: count(coverage.attemptedPages),
    }
  }, [coverage, language])

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
      <PaperCardHeader
        title={t('settings.linkPreviewsTitle')}
        right={<SettingsSavedChip visible={savedVisible} />}
      />
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
                disabled={!fetchEnabled || settings.fetchMode !== 'background'}
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
            onBlur={onCommitBlocklist}
            rows={4}
            placeholder={t('settings.linkPreviewsBlocklistPlaceholder')}
            data-testid="link-previews-blocklist-input"
            className={cn(
              'border-border-default rounded-paper bg-paper w-full resize-y border px-3 py-2 font-mono text-[11.5px] text-ink',
              'focus:border-accent focus:outline-none',
            )}
          />
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

        <Field label={t('settings.linkPreviewsCoverageLabel')}>
          <p
            className="text-ink-muted m-0 font-mono text-[11.5px]"
            data-testid="link-previews-coverage"
          >
            {coverage === null
              ? coverageFailed
                ? t('settings.linkPreviewsCoverageError')
                : t('settings.linkPreviewsCoverageLoading')
              : !coverageView
                ? t('settings.linkPreviewsCoverageEmpty')
                : coverageView.hasAttempts
                  ? t('settings.linkPreviewsCoverageValue', {
                      percent: coverageView.percent,
                      withImage: coverageView.withImage,
                      eligible: coverageView.eligible,
                    })
                  : t('settings.linkPreviewsCoverageNotFetched')}
          </p>
          {coverageView?.hasAttempts ? (
            <p
              className="text-ink-faint m-0 mt-0.5 font-mono text-[11px]"
              data-testid="link-previews-coverage-rate"
            >
              {t('settings.linkPreviewsCoverageRate', {
                rate: coverageView.successRate,
                checked: coverageView.checked,
              })}
            </p>
          ) : null}
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
