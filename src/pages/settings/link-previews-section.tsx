/**
 * Settings → Link previews section.
 *
 * Surfaces the user-facing knobs for the og:image cache that the v0.3
 * paper Browse card mode populates lazily:
 * - Fetch toggle (mirrors AppConfig.ogImage.fetchEnabled — default on).
 * - Live cache footprint (rows · blobs · bytes).
 * - "Run cleanup now" + "Clear all link previews" actions.
 *
 * Deferred for a follow-up:
 * - Per-domain blocklist editor.
 * - Eviction-mode picker (off / time / size / LRU) + numeric input.
 *
 * Persistence:
 * - The toggle writes through `saveConfig` from the shell-data context
 *   so all routes see the new value immediately.
 * - Stats are reloaded after cleanup / clear so the user sees the
 *   storage drop without needing to refresh manually.
 */

import { useCallback, useEffect, useState } from 'react'
import { useI18n } from '@/lib/i18n'
import { useShellData } from '@/app/shell-data-context'
import { backend } from '@/lib/backend-client'
import { formatBytes } from '@/lib/format'
import { PaperCard, PaperCardBody, PaperCardHeader } from '@/components/cards'
import type { OgImageStorageStats } from '@/lib/types'
import { cn } from '@/lib/cn'

export interface LinkPreviewsSectionProps {
  anchorId?: string
}

export function LinkPreviewsSection({
  anchorId = 'link-previews',
}: LinkPreviewsSectionProps) {
  const { language, t } = useI18n()
  const { snapshot, saveConfig } = useShellData()
  const [stats, setStats] = useState<OgImageStorageStats | null>(null)
  const [pendingAction, setPendingAction] = useState<
    'cleanup' | 'clear' | null
  >(null)
  const [summary, setSummary] = useState<string | null>(null)

  const fetchEnabled = snapshot?.config.ogImage?.fetchEnabled ?? true

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

  const onToggleFetch = async (next: boolean) => {
    if (!snapshot) return
    const currentOgImage = snapshot.config.ogImage ?? {
      fetchEnabled: true,
      blockedHosts: [],
      cleanup: { mode: 'off' as const },
    }
    await saveConfig({
      ...snapshot.config,
      ogImage: { ...currentOgImage, fetchEnabled: next },
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

  return (
    <PaperCard testId="settings-link-previews-section">
      <span id={anchorId} aria-hidden />
      <PaperCardHeader title={t('settings.linkPreviewsTitle')} />
      <PaperCardBody>
        <p className="m-0 mb-4 font-serif text-[13.5px] italic leading-[1.55] text-ink-muted">
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

        <Field label={t('settings.linkPreviewsStatsLabel')}>
          <p
            className="m-0 font-mono text-[11.5px] text-ink-muted"
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
                'border-border-default rounded-paper border px-3 py-1.5 font-sans text-[12px] text-ink transition-colors',
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
                'border-border-default rounded-paper border px-3 py-1.5 font-sans text-[12px] text-ink-muted transition-colors',
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
                className="font-mono text-[10.5px] text-ink-faint"
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

interface FieldProps {
  label: string
  help?: string
  children: React.ReactNode
}

function Field({ label, help, children }: FieldProps) {
  return (
    <div className="border-border-light py-3 first:pt-0 last:pb-0 last:border-b-0 border-b">
      <div className="mb-2 font-mono text-[10px] uppercase tracking-[0.08em] text-ink-faint">
        {label}
      </div>
      <div className="flex items-center justify-between gap-4">
        <div className="flex-1">
          {help ? (
            <p className="m-0 mb-2 font-sans text-[12px] text-ink-muted">
              {help}
            </p>
          ) : null}
          {children}
        </div>
      </div>
    </div>
  )
}

interface ToggleProps {
  value: boolean
  onChange: (value: boolean) => void
  onLabel: string
  offLabel: string
  testId?: string
}

function Toggle({ value, onChange, onLabel, offLabel, testId }: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={value}
      onClick={() => onChange(!value)}
      data-testid={testId}
      className={cn(
        'border-border-default inline-flex items-center gap-3 border px-3 py-1.5 text-[12px] transition-colors rounded-paper',
        value
          ? 'border-accent bg-accent-soft text-accent-text'
          : 'text-ink-muted hover:border-ink-muted hover:bg-hover',
      )}
    >
      <span
        className={cn(
          'inline-block h-3 w-3 rounded-full',
          value ? 'bg-accent' : 'bg-ink-faint',
        )}
      />
      {value ? onLabel : offLabel}
    </button>
  )
}
