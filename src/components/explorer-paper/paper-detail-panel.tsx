/**
 * Detail slide-over that surfaces a single archived visit.
 *
 * Mounts above the Browse contact sheet (or any list-of-entries surface)
 * when a row is selected. The panel is 460 px wide with a soft backdrop;
 * Escape and a backdrop click close it.
 *
 * ## Responsibilities
 * - Render the title, URL, and the standard action row (Open · Copy URL ·
 *   Refind · Export).
 * - Render the visit summary (first / last visit, total visits, typed-count)
 *   and a small per-visit history sparkline.
 * - Render the provenance section (source profile, transition kind,
 *   captured-in-run hash) and an optional title-version history.
 * - Host the user's Notes textarea (debounced via the parent) and Tag chip
 *   editor with add/remove handlers.
 * - Render the "Look further" related list: page-level insights, domain
 *   roll-up, thread membership, session anchor.
 *
 * ## Not responsible for
 * - Annotations persistence — caller supplies `notes` + `tags` and the
 *   parent debounces writes back to the backend (or localStorage in the
 *   prototype window).
 * - Refind / Export action execution — caller wires its own handlers.
 *
 * ## Dependencies
 * - Paper tokens via `src/styles/tokens.css`.
 */

import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
} from 'react'
import { cn } from '@/lib/cn'
import { sanitizeExplorerDisplayText } from '@/pages/explorer/helpers'

export interface PaperDetailPanelCopy {
  /** Mono uppercase eyebrow above the title, e.g. "RECORD". */
  recordEyebrow: string
  closeLabel: string
  openAction: string
  copyAction: string
  refindAction: string
  exportAction: string
  /** Section headings ("Provenance", "Your notes", "Tags", "Look further"). */
  provenanceHeading: string
  notesHeading: string
  tagsHeading: string
  lookFurtherHeading: string
  /** Detail field labels. */
  firstVisitLabel: string
  lastVisitLabel: string
  totalVisitsLabel: string
  typedCountLabel: string
  recentVisitsLabel: string
  sourceLabel: string
  transitionLabel: string
  capturedInRunLabel: string
  titleHistoryLabel: string
  /** Notes meta. */
  notesPlaceholder: string
  notesEmpty: string
  notesSavedLocally: string
  /** Singular char counter, e.g. "1 char". */
  notesCharSingular: string
  /** Plural char counter, e.g. "{count} chars". */
  notesCharPlural: string
  /** Tag chip helpers. */
  tagInputPlaceholder: string
  /** Aria-label template for the remove-tag button, with `{tag}` placeholder. */
  tagRemoveAriaLabel: string
  /** Look-further row labels. */
  pageLevelInsights: string
  allOfDomain: string
  threadLabel: string
  sessionLabel: string
  /** Visit history row count suffix, e.g. "{count}×". */
  visitCountSuffix: string
}

export interface PaperDetailPanelVisitHistoryRow {
  date: string
  count: number
}

export interface PaperDetailPanelTitleVersion {
  date: string
  title: string
}

export interface PaperDetailPanelEntry {
  id: number | string
  title: string
  url: string
  domain: string
  /** Formatted "first visit" timestamp, e.g. "2025-11-04 09:17". */
  firstVisitAt?: string
  /** Formatted "last visit" timestamp. */
  lastVisitAt?: string
  visitCount?: number
  typedCount?: number
  /** Free-form source label, e.g. "Chrome / Default profile". */
  source?: string
  /** Free-form transition label, e.g. "link" / "typed" / "auto-bookmark". */
  transition?: string
  /** "Captured in run" identifier + timestamp string. */
  capturedIn?: string
  visitHistory?: PaperDetailPanelVisitHistoryRow[]
  titleVersions?: PaperDetailPanelTitleVersion[]
}

export interface PaperDetailPanelLookFurtherCounts {
  visitsLabel?: string
  domainPagesLabel?: string
  threadLabel?: string
  sessionLabel?: string
}

export interface PaperDetailPanelProps {
  entry: PaperDetailPanelEntry | null
  notes: string
  tags: string[]
  onClose: () => void
  onOpen?: (entry: PaperDetailPanelEntry) => void
  onCopyUrl?: (entry: PaperDetailPanelEntry) => void
  onRefind?: (entry: PaperDetailPanelEntry) => void
  onExport?: (entry: PaperDetailPanelEntry) => void
  onUpdateNotes: (next: string) => void
  onUpdateTags: (next: string[]) => void
  /** Look-further row click handlers. Optional — rows render but become inert when omitted. */
  onOpenInsights?: (entry: PaperDetailPanelEntry) => void
  onOpenDomain?: (entry: PaperDetailPanelEntry) => void
  onOpenThread?: (entry: PaperDetailPanelEntry) => void
  onOpenSession?: (entry: PaperDetailPanelEntry) => void
  lookFurtherCounts?: PaperDetailPanelLookFurtherCounts
  /** Optional debounce window for notes writes. Default 400 ms. Set 0 to write synchronously. */
  notesDebounceMs?: number
  copy: PaperDetailPanelCopy
  className?: string
  testId?: string
}

export function PaperDetailPanel({
  entry,
  notes,
  tags,
  onClose,
  onOpen,
  onCopyUrl,
  onRefind,
  onExport,
  onUpdateNotes,
  onUpdateTags,
  onOpenInsights,
  onOpenDomain,
  onOpenThread,
  onOpenSession,
  lookFurtherCounts,
  notesDebounceMs = 400,
  copy,
  className,
  testId,
}: PaperDetailPanelProps) {
  const [notesValue, setNotesValue] = useState(notes)
  const [tagInput, setTagInput] = useState('')
  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  // Holds the timeout callback so close/unmount/entry-switch can flush the
  // still-debounced edit synchronously without losing the closure that
  // captures the previous record's `onUpdateNotes` (which is bound to that
  // record's url at the parent). Auto-save means auto-save: the panel has
  // no explicit "discard" affordance, so closing should commit, not drop.
  const pendingFlushRef = useRef<(() => void) | null>(null)
  const [savePending, setSavePending] = useState(false)
  // Track the prop values we synced from last so we can adjust state during
  // render when they change. This is the React 19-blessed alternative to
  // setState-in-effect for "derive state from a prop change".
  const [trackedEntryId, setTrackedEntryId] = useState(entry?.id ?? null)
  const [lastSyncedNotes, setLastSyncedNotes] = useState(notes)

  const nextEntryId = entry?.id ?? null
  if (nextEntryId !== trackedEntryId) {
    // Swap in the new record's notes during render (React 19 supports
    // setState during render to derive state from a prop change). The
    // pending-flush ref is cleared in the layout effect below so we
    // never mutate refs during render.
    setTrackedEntryId(nextEntryId)
    setNotesValue(notes)
    setLastSyncedNotes(notes)
  } else if (notes !== lastSyncedNotes && !savePending) {
    // External notes update (backend refresh) and no in-flight edit pending.
    setNotesValue(notes)
    setLastSyncedNotes(notes)
  }

  // Flush any pending edit to the *previous* record's onUpdateNotes the
  // moment trackedEntryId changes. Using useLayoutEffect keeps the flush
  // synchronous with the swap so the new record never briefly shows the
  // old text. The pending-flush closure captures the previous record's
  // onUpdateNotes by reference, so it lands on the right entry.
  useLayoutEffect(() => {
    if (pendingFlushRef.current) {
      if (saveTimer.current) clearTimeout(saveTimer.current)
      saveTimer.current = null
      const flush = pendingFlushRef.current
      pendingFlushRef.current = null
      flush()
    }
  }, [trackedEntryId])

  useEffect(
    () => () => {
      if (saveTimer.current) {
        clearTimeout(saveTimer.current)
        saveTimer.current = null
      }
      if (pendingFlushRef.current) {
        const flush = pendingFlushRef.current
        pendingFlushRef.current = null
        flush()
      }
    },
    [],
  )

  useEffect(() => {
    function handleKey(event: globalThis.KeyboardEvent) {
      if (event.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', handleKey)
    return () => window.removeEventListener('keydown', handleKey)
  }, [onClose])

  const handleNotesChange = useCallback(
    (next: string) => {
      setNotesValue(next)
      if (saveTimer.current) clearTimeout(saveTimer.current)
      // One closure shared between the debounce timer and the
      // unmount/entry-switch flush path; both fire-paths capture the
      // current `onUpdateNotes` so the edit always lands on the record
      // the user was looking at when they typed.
      const flush = () => {
        saveTimer.current = null
        pendingFlushRef.current = null
        setSavePending(false)
        setLastSyncedNotes(next)
        onUpdateNotes(next)
      }
      if (notesDebounceMs <= 0) {
        flush()
        return
      }
      pendingFlushRef.current = flush
      setSavePending(true)
      saveTimer.current = setTimeout(flush, notesDebounceMs)
    },
    [notesDebounceMs, onUpdateNotes],
  )

  const handleAddTag = useCallback(() => {
    const candidate = tagInput.trim().toLowerCase()
    if (!candidate) return
    if (tags.includes(candidate)) {
      setTagInput('')
      return
    }
    onUpdateTags([...tags, candidate])
    setTagInput('')
  }, [tagInput, tags, onUpdateTags])

  const handleRemoveTag = useCallback(
    (value: string) => {
      onUpdateTags(tags.filter((tag) => tag !== value))
    },
    [tags, onUpdateTags],
  )

  const handleTagKeyDown = useCallback(
    (event: KeyboardEvent<HTMLInputElement>) => {
      if (event.key === 'Enter') {
        event.preventDefault()
        handleAddTag()
        return
      }
      if (
        event.key === 'Backspace' &&
        tagInput.length === 0 &&
        tags.length > 0
      ) {
        event.preventDefault()
        handleRemoveTag(tags[tags.length - 1])
      }
    },
    [handleAddTag, handleRemoveTag, tagInput, tags],
  )

  if (!entry) return null

  const visitHistory = entry.visitHistory ?? []
  const maxVisit = visitHistory.reduce(
    (acc, row) => Math.max(acc, row.count),
    1,
  )

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby={`${entry.id}-detail-title`}
      data-testid={testId}
      className="fixed inset-0 z-[1000] flex justify-end"
    >
      <button
        type="button"
        aria-label={copy.closeLabel}
        onClick={onClose}
        className={cn(
          'absolute inset-0 bg-[rgba(28,24,20,0.16)] cursor-default',
          'dark:bg-[rgba(0,0,0,0.4)]',
          'motion-safe:animate-[paper-detail-backdrop-in_180ms_ease-out]',
        )}
      />

      <aside
        className={cn(
          'relative h-full w-[460px] max-w-[90vw] overflow-y-auto',
          'bg-paper border-border-default border-l',
          'motion-safe:animate-[paper-detail-slide-in_220ms_cubic-bezier(0.22,0.61,0.36,1)]',
          className,
        )}
      >
        <header className="border-border-light flex items-start justify-between border-b px-6 pb-4 pt-5">
          <span className="text-ink-faint font-mono text-[9.5px] uppercase tracking-[0.08em]">
            {copy.recordEyebrow}
          </span>
          <button
            type="button"
            onClick={onClose}
            title={copy.closeLabel}
            aria-label={copy.closeLabel}
            className="border-border-default text-ink-muted hover:border-ink-muted hover:text-ink rounded-paper inline-flex h-7 w-7 shrink-0 items-center justify-center border transition-colors duration-150"
          >
            ✕
          </button>
        </header>

        <div className="px-6 pb-8 pt-5">
          <h2
            id={`${entry.id}-detail-title`}
            className="text-ink m-0 font-serif text-[20px] font-medium leading-[1.3] tracking-[-0.01em]"
          >
            {sanitizeExplorerDisplayText(entry.title || entry.url, 200)}
          </h2>
          <a
            className="text-accent-text mt-2 block break-all font-mono text-[11.5px] leading-[1.4]"
            href={entry.url}
            target="_blank"
            rel="noreferrer noopener"
          >
            {sanitizeExplorerDisplayText(entry.url, 200)}
          </a>

          <div className="mt-[14px] flex flex-wrap gap-[6px]">
            <DetailAction
              variant="primary"
              label={copy.openAction}
              onClick={onOpen ? () => onOpen(entry) : undefined}
            />
            <DetailAction
              label={copy.copyAction}
              onClick={onCopyUrl ? () => onCopyUrl(entry) : undefined}
            />
            <DetailAction
              label={copy.refindAction}
              onClick={onRefind ? () => onRefind(entry) : undefined}
            />
            <DetailAction
              label={copy.exportAction}
              onClick={onExport ? () => onExport(entry) : undefined}
            />
          </div>

          <Divider />

          <div className="flex gap-5">
            <DetailField label={copy.firstVisitLabel}>
              <Mono>{entry.firstVisitAt ?? '—'}</Mono>
            </DetailField>
            <DetailField label={copy.lastVisitLabel}>
              <Mono>{entry.lastVisitAt ?? '—'}</Mono>
            </DetailField>
          </div>
          <div className="mt-[14px] flex gap-5">
            <DetailField label={copy.totalVisitsLabel}>
              <Mono>{entry.visitCount?.toLocaleString() ?? '—'}</Mono>
            </DetailField>
            <DetailField label={copy.typedCountLabel}>
              <Mono>{entry.typedCount?.toLocaleString() ?? '0'}</Mono>
            </DetailField>
          </div>

          {visitHistory.length > 0 ? (
            <div className="mt-[14px]">
              <DetailLabel>{copy.recentVisitsLabel}</DetailLabel>
              <div
                className="mt-[6px] flex flex-col gap-1"
                data-testid="paper-detail-visit-history"
              >
                {visitHistory.map((row) => (
                  <div
                    key={row.date}
                    className="text-ink-muted grid grid-cols-[70px_1fr_30px] items-center gap-2 font-mono text-[10.5px]"
                  >
                    <span className="text-ink-faint">{row.date}</span>
                    <span
                      aria-hidden="true"
                      className="block h-[6px] rounded-[1px] bg-[color-mix(in_srgb,var(--accent)_25%,var(--bg-page))]"
                      style={{
                        width: `${(row.count / maxVisit) * 100}%`,
                      }}
                    />
                    <span className="text-ink-secondary text-right">
                      {copy.visitCountSuffix.replace(
                        '{count}',
                        String(row.count),
                      )}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          ) : null}

          <Divider />

          <SectionTitle>{copy.provenanceHeading}</SectionTitle>
          <DetailField label={copy.sourceLabel}>
            <span className="text-ink-secondary font-sans text-[13px]">
              {entry.source ?? '—'}
            </span>
          </DetailField>
          <div className="mt-[14px]">
            <DetailField label={copy.transitionLabel}>
              <span className="text-ink-secondary font-sans text-[13px]">
                {entry.transition ?? '—'}
              </span>
            </DetailField>
          </div>
          {entry.capturedIn ? (
            <div className="mt-[14px]">
              <DetailField label={copy.capturedInRunLabel}>
                <Mono className="text-ink-faint">{entry.capturedIn}</Mono>
              </DetailField>
            </div>
          ) : null}

          {entry.titleVersions && entry.titleVersions.length > 0 ? (
            <div className="mt-[14px]">
              <DetailLabel>{copy.titleHistoryLabel}</DetailLabel>
              <div className="mt-[6px] flex flex-col gap-[6px]">
                {entry.titleVersions.map((version) => (
                  <div key={`${version.date}-${version.title}`}>
                    <Mono className="text-ink-faint text-[10px]">
                      {version.date}
                    </Mono>
                    <div className="text-ink-secondary font-serif text-[12.5px] italic">
                      {version.title}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          ) : null}

          <Divider />

          <SectionTitle>{copy.notesHeading}</SectionTitle>
          <textarea
            data-testid="paper-detail-notes"
            value={notesValue}
            onChange={(event) => handleNotesChange(event.target.value)}
            placeholder={copy.notesPlaceholder}
            className={cn(
              'border-border-default bg-card-paper text-ink',
              'rounded-paper w-full resize-y border px-3 py-[10px]',
              'font-serif text-[13.5px] italic leading-[1.5]',
              'min-h-[70px]',
              'placeholder:text-ink-faint placeholder:italic',
              'focus:border-accent focus:bg-paper focus:outline-none',
              'transition-colors duration-150',
            )}
          />
          <div className="text-ink-faint mt-1 flex justify-between font-mono text-[9.5px]">
            <span>
              {notesValue
                ? notesValue.length === 1
                  ? copy.notesCharSingular
                  : copy.notesCharPlural.replace(
                      '{count}',
                      String(notesValue.length),
                    )
                : copy.notesEmpty}
            </span>
            <span>{notesValue ? copy.notesSavedLocally : ''}</span>
          </div>

          <SectionTitle className="mt-[18px]">{copy.tagsHeading}</SectionTitle>
          <div className="mt-[6px] flex flex-wrap gap-[6px]">
            {tags.map((tag) => (
              <span
                key={tag}
                data-testid={`paper-detail-tag-${tag}`}
                className="border-border-default bg-card-paper text-ink-secondary rounded-pill inline-flex items-center gap-1 border px-[9px] py-[3px] font-mono text-[10.5px] tracking-[0.01em]"
              >
                {tag}
                <button
                  type="button"
                  aria-label={copy.tagRemoveAriaLabel.replace('{tag}', tag)}
                  onClick={() => handleRemoveTag(tag)}
                  className="text-ink-faint hover:text-error ml-[2px] px-[1px] text-[10px]"
                >
                  ✕
                </button>
              </span>
            ))}
            <input
              data-testid="paper-detail-tag-input"
              type="text"
              value={tagInput}
              onChange={(event) => setTagInput(event.target.value)}
              onKeyDown={handleTagKeyDown}
              placeholder={copy.tagInputPlaceholder}
              className={cn(
                'border-border-default text-ink-faint focus:border-accent focus:text-ink',
                'rounded-pill inline-flex items-center bg-transparent px-[9px] py-[3px]',
                'border border-dashed font-mono text-[10.5px] outline-none',
                'w-[90px] min-w-[70px]',
                'focus:border-solid',
              )}
            />
          </div>

          <Divider />

          <SectionTitle>{copy.lookFurtherHeading}</SectionTitle>
          <div className="mt-[4px] flex flex-col">
            <LookFurtherRow
              label={copy.pageLevelInsights}
              hint={lookFurtherCounts?.visitsLabel}
              onClick={onOpenInsights ? () => onOpenInsights(entry) : undefined}
            />
            <LookFurtherRow
              label={copy.allOfDomain.replace('{domain}', entry.domain)}
              hint={lookFurtherCounts?.domainPagesLabel}
              onClick={onOpenDomain ? () => onOpenDomain(entry) : undefined}
            />
            <LookFurtherRow
              label={copy.threadLabel}
              hint={lookFurtherCounts?.threadLabel}
              onClick={onOpenThread ? () => onOpenThread(entry) : undefined}
            />
            <LookFurtherRow
              label={copy.sessionLabel}
              hint={lookFurtherCounts?.sessionLabel}
              onClick={onOpenSession ? () => onOpenSession(entry) : undefined}
            />
          </div>
        </div>
      </aside>
    </div>
  )
}

function DetailAction({
  variant = 'default',
  label,
  onClick,
}: {
  variant?: 'default' | 'primary'
  label: string
  onClick?: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={!onClick}
      className={cn(
        'rounded-paper inline-flex items-center gap-[5px] border px-[10px] py-[5px]',
        'font-sans text-[11.5px] transition-colors duration-150',
        variant === 'primary'
          ? 'border-accent text-accent hover:bg-accent-soft'
          : 'border-border-default text-ink-secondary bg-card-paper hover:border-ink-muted hover:text-ink',
        'disabled:cursor-not-allowed disabled:opacity-40',
      )}
    >
      {label}
    </button>
  )
}

function DetailField({
  label,
  children,
}: {
  label: string
  children: React.ReactNode
}) {
  return (
    <div className="min-w-0 flex-1">
      <DetailLabel>{label}</DetailLabel>
      <div className="mt-[3px]">{children}</div>
    </div>
  )
}

function DetailLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="text-ink-faint font-mono text-[9.5px] font-medium uppercase tracking-[0.08em]">
      {children}
    </div>
  )
}

function Mono({
  children,
  className,
}: {
  children: React.ReactNode
  className?: string
}) {
  return (
    <span className={cn('text-ink-secondary font-mono text-[12px]', className)}>
      {children}
    </span>
  )
}

function Divider() {
  return <div className="bg-border-light my-[18px] h-px w-full" />
}

function SectionTitle({
  children,
  className,
}: {
  children: React.ReactNode
  className?: string
}) {
  return (
    <h3
      className={cn(
        'text-ink-muted m-0 mb-[10px] font-serif text-[12px] italic',
        className,
      )}
    >
      {children}
    </h3>
  )
}

function LookFurtherRow({
  label,
  hint,
  onClick,
}: {
  label: string
  hint?: string
  onClick?: () => void
}) {
  const isInteractive = Boolean(onClick)
  const Element = isInteractive ? 'button' : 'div'
  return (
    <Element
      type={isInteractive ? 'button' : undefined}
      onClick={onClick}
      className={cn(
        'border-border-light flex items-center justify-between border-b py-2 last:border-b-0',
        'font-sans text-[12px] text-ink-secondary text-left',
        isInteractive
          ? 'hover:text-accent cursor-pointer transition-colors duration-150'
          : 'cursor-default',
      )}
    >
      <span>{label}</span>
      {hint ? (
        <span className="text-ink-faint font-mono text-[10.5px]">{hint}</span>
      ) : null}
    </Element>
  )
}
