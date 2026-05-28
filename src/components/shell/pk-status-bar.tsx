/**
 * Paper-redesign bottom status bar.
 *
 * Why this file exists:
 * - Replaces the legacy footerless shell. The design uses the status bar as
 *   ambient telemetry: archive size, span, source switcher, last archived,
 *   plus an epigraph that rotates per session.
 *
 * Responsibilities:
 * - Render archive status + counts + source-switcher trigger + epigraph.
 * - Delegate source selection to a Popover that returns the chosen source id.
 *
 * Not responsible for:
 * - Backing data fetch (parent shell composes from snapshot).
 * - Persisting source filter (parent owns).
 */

import { useMemo } from 'react'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { useI18n } from '@/lib/i18n/hooks'
import { cn } from '@/lib/cn'

export interface PKStatusBarSource {
  id: string
  label: string
  profile?: string | null
  color: string
  /**
   * Per-source page count. Optional because the backend's
   * `BrowserProfile.historyBytes` is *bytes on disk* (not a row count),
   * and we don't currently surface a per-profile visit total.
   * Earlier versions of this prop received `historyBytes` here and
   * displayed it as "pages" — a visible-to-the-user inaccuracy. Until
   * the backend exposes a real per-profile row count, leave undefined
   * and the picker shows only the byte size.
   */
  pages?: number
  size: string
}

export interface PKStatusBarProps {
  archiving: boolean
  initialized: boolean
  totalPages: number | null
  totalSize: string | null
  sinceLabel: string | null
  lastArchivedLabel: string | null
  sources: PKStatusBarSource[]
  selectedSourceId: string | null
  onSelectSource: (id: string | null) => void
  onManageSources: () => void
  /** Stable epigraph index, persisted across renders by the parent. */
  epigraphIndex?: number
}

export function PKStatusBar({
  archiving,
  initialized,
  totalPages,
  totalSize,
  sinceLabel,
  lastArchivedLabel,
  sources,
  selectedSourceId,
  onSelectSource,
  onManageSources,
  epigraphIndex,
}: PKStatusBarProps) {
  const { t } = useI18n()

  const epigraph = useMemo(() => {
    const pool: string[] = [
      t('shell.epigraph1'),
      t('shell.epigraph2'),
      t('shell.epigraph3'),
      t('shell.epigraph4'),
      t('shell.epigraph5'),
      t('shell.epigraph6'),
    ]
    const idx =
      typeof epigraphIndex === 'number'
        ? Math.abs(epigraphIndex) % pool.length
        : 0
    return pool[idx]
  }, [epigraphIndex, t])

  const totalPagesLabel =
    totalPages !== null ? new Intl.NumberFormat().format(totalPages) : null
  const activeSource = selectedSourceId
    ? (sources.find((s) => s.id === selectedSourceId) ?? null)
    : null
  const sourceTriggerLabel = activeSource
    ? `${activeSource.label}${activeSource.profile ? ` · ${activeSource.profile}` : ''}`
    : t(
        sources.length === 1
          ? 'shell.sourcesCountSingular'
          : 'shell.sourcesCountPlural',
        { count: sources.length },
      )

  return (
    <footer
      className="border-border-light bg-paper flex h-[32px] shrink-0 items-center gap-3 border-t px-4 text-[11px] text-ink-muted"
      data-testid="pk-status-bar"
    >
      <span className="flex items-center gap-1.5">
        <span
          className={cn(
            'inline-block h-1.5 w-1.5 rounded-full',
            archiving
              ? 'bg-accent animate-pulse'
              : initialized
                ? 'bg-success'
                : 'bg-ink-faint',
          )}
        />
        <span>
          {archiving
            ? t('shell.archiving')
            : initialized
              ? t('shell.archiveKept')
              : t('shell.archiveNotInitialized')}
        </span>
      </span>

      {totalPagesLabel && totalSize ? (
        <>
          <Sep />
          <span>
            {totalPagesLabel} {t('shell.pages')} · {totalSize}
          </span>
        </>
      ) : null}

      {sinceLabel ? (
        <>
          <Sep />
          <span>{sinceLabel}</span>
        </>
      ) : null}

      {sources.length > 0 ? (
        <>
          <Sep />
          <Popover>
            <PopoverTrigger asChild>
              <button
                type="button"
                className="hover:text-ink-secondary flex items-center gap-1.5 transition-colors"
                data-testid="pk-status-bar-source-trigger"
              >
                <span className="flex items-center gap-[2px]">
                  {activeSource ? (
                    <span
                      className="inline-block h-1.5 w-1.5 rounded-full"
                      style={{ background: activeSource.color }}
                    />
                  ) : (
                    sources
                      .slice(0, 4)
                      .map((source) => (
                        <span
                          key={source.id}
                          className="inline-block h-1.5 w-1.5 rounded-full"
                          style={{ background: source.color }}
                        />
                      ))
                  )}
                </span>
                <span>{sourceTriggerLabel}</span>
              </button>
            </PopoverTrigger>
            <PopoverContent
              side="top"
              align="start"
              sideOffset={10}
              className="min-w-[280px] p-0"
            >
              <div className="border-border-light flex items-center justify-between border-b px-4 py-3">
                <span className="font-serif text-[14px] font-medium text-ink">
                  {t('shell.sourcesTitle')}
                </span>
                <span className="font-mono text-[10px] text-ink-faint">
                  {t('shell.sourcesConnected', { count: sources.length })}
                </span>
              </div>
              <button
                type="button"
                onClick={() => onSelectSource(null)}
                className={cn(
                  'flex w-full items-center gap-3 px-4 py-2 text-left transition-colors hover:bg-hover',
                  !selectedSourceId ? 'bg-accent-soft' : '',
                )}
              >
                <span className="flex items-center gap-[2px]">
                  {sources.slice(0, 4).map((source) => (
                    <span
                      key={source.id}
                      className="inline-block h-1.5 w-1.5 rounded-full"
                      style={{ background: source.color }}
                    />
                  ))}
                </span>
                <span className="flex-1">
                  <span className="block font-sans text-[12.5px] text-ink">
                    {t('shell.sourcesAll')}
                  </span>
                  <span className="block font-mono text-[10px] text-ink-faint">
                    {totalPagesLabel ?? sources.length} {t('shell.pages')}
                    {totalSize ? ` · ${totalSize}` : ''}
                  </span>
                </span>
                {!selectedSourceId && <span className="text-accent">✓</span>}
              </button>
              <div className="border-border-light border-t" />
              {sources.map((source) => (
                <button
                  key={source.id}
                  type="button"
                  onClick={() =>
                    onSelectSource(
                      selectedSourceId === source.id ? null : source.id,
                    )
                  }
                  className={cn(
                    'flex w-full items-center gap-3 px-4 py-2 text-left transition-colors hover:bg-hover',
                    selectedSourceId === source.id ? 'bg-accent-soft' : '',
                  )}
                >
                  <span
                    className="inline-block h-1.5 w-1.5 rounded-full"
                    style={{ background: source.color }}
                  />
                  <span className="flex-1">
                    <span className="block font-sans text-[12.5px] text-ink">
                      {source.label}
                      {source.profile ? (
                        <span className="text-ink-faint">
                          {' '}
                          · {source.profile}
                        </span>
                      ) : null}
                    </span>
                    <span className="block font-mono text-[10px] text-ink-faint">
                      {typeof source.pages === 'number' && source.pages > 0
                        ? `${new Intl.NumberFormat().format(source.pages)} ${t('shell.pages')} · ${source.size}`
                        : source.size}
                    </span>
                  </span>
                  {selectedSourceId === source.id && (
                    <span className="text-accent">✓</span>
                  )}
                </button>
              ))}
              <div className="border-border-light border-t px-4 py-2">
                <button
                  type="button"
                  onClick={onManageSources}
                  className="text-accent hover:underline font-sans text-[12px]"
                >
                  {t('shell.manageSources')}
                </button>
              </div>
            </PopoverContent>
          </Popover>
        </>
      ) : null}

      {lastArchivedLabel ? (
        <>
          <Sep />
          <span>{lastArchivedLabel}</span>
        </>
      ) : null}

      <span className="ml-auto truncate font-serif text-[11.5px] italic text-ink-faint">
        {epigraph}
      </span>
    </footer>
  )
}

function Sep() {
  return <span aria-hidden className="bg-border-light inline-block h-3 w-px" />
}
