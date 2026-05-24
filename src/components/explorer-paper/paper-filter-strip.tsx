/**
 * Compact filter chip strip + add-filter popover for the paper Browse view.
 *
 * ## Responsibilities
 * - Render an inline strip of the currently-active filters as removable
 *   chips ("domain: github.com ×", "profile: Chrome / Default ×").
 * - Expose an "Add filter" trigger that opens a small popover form
 *   carrying the additional filter dimensions the chip strip can't show
 *   inline (domain text, browser select, date range).
 * - Expose a "Clear all" affordance once at least one chip is active.
 *
 * ## Not responsible for
 * - Owning URL state — the route reads `activeFilters` from
 *   `useExplorerUrlState` and wires `onRemove` / `onAdd` / `onClearAll`
 *   to its existing `updateParam` / `clearAllFilters` helpers.
 * - Deciding which filter shape applies to the current query (the route
 *   already validates regex syntax and clamps date inputs).
 *
 * ## Dependencies
 * - Paper tokens via `src/styles/tokens.css`.
 */

import { useEffect, useRef, useState } from 'react'
import { cn } from '@/lib/cn'

export interface PaperFilterChip {
  /** Stable URL-param key — used as the `onRemove` argument. */
  id: string
  /** Localised dimension label, e.g. "Domain", "域". */
  label: string
  /** Current value the chip shows next to the label, e.g. "github.com". */
  value: string
}

export interface PaperFilterStripCopy {
  /** Trigger label, e.g. "+ Filter" / "+ 筛选". */
  addFilter: string
  /** "Clear all" link copy. */
  clearAll: string
  /** Template for the chip's `aria-label`, e.g. "Remove filter {label}: {value}". */
  removeFilterAria: string
  /** Empty-state hint shown next to the trigger when no chip is active. */
  emptyHint: string
  /** Popover heading. */
  popoverTitle: string
  /** Form-field labels. */
  fieldDomain: string
  fieldBrowser: string
  fieldProfile: string
  fieldStart: string
  fieldEnd: string
  fieldRegex: string
  /** Select-all option for browser / profile drop-downs. */
  selectAllBrowsers: string
  selectAllProfiles: string
  /** Apply button on the popover form. */
  applyLabel: string
  /** Close button aria-label. */
  closeLabel: string
}

export interface PaperFilterStripOption {
  value: string
  label: string
}

export interface PaperFilterStripFormState {
  domain: string
  browserKind: string
  profileId: string
  start: string
  end: string
  regexMode: boolean
}

export interface PaperFilterStripProps {
  chips: PaperFilterChip[]
  copy: PaperFilterStripCopy
  /** Form values mirrored from URL state so the popover edits the same fields the chips display. */
  formState: PaperFilterStripFormState
  browserOptions: PaperFilterStripOption[]
  profileOptions: PaperFilterStripOption[]
  /** Fired when the user clicks the × on a chip. Receives the chip's `id`. */
  onRemove: (id: string) => void
  /** Fired when the user clicks "Clear all". */
  onClearAll: () => void
  /** Fired when the popover form is submitted. The route applies the diff. */
  onApply: (next: PaperFilterStripFormState) => void
  className?: string
  testId?: string
}

export function PaperFilterStrip({
  chips,
  copy,
  formState,
  browserOptions,
  profileOptions,
  onRemove,
  onClearAll,
  onApply,
  className,
  testId,
}: PaperFilterStripProps) {
  const [open, setOpen] = useState(false)
  const containerRef = useRef<HTMLDivElement | null>(null)
  const [draft, setDraft] = useState(formState)

  // Sync the draft from the URL-derived formState ONLY when the popover
  // is closed. Otherwise a chip removal (or any external URL update)
  // would clobber whatever the user has half-typed into the popover form
  // — silent data loss. When the popover reopens, the draft is re-seeded
  // from the latest formState below.
  useEffect(() => {
    if (open) return
    setDraft(formState)
  }, [formState, open])
  // Reseed the draft on each open so the form reflects the current URL
  // state (in case it shifted while the popover was closed).
  useEffect(() => {
    if (open) setDraft(formState)
    // formState intentionally excluded — we only want to reseed on the
    // transition from closed → open, not on every formState change while
    // the popover is open.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open])

  useEffect(() => {
    if (!open) return
    function handlePointer(event: MouseEvent) {
      if (!containerRef.current) return
      if (!containerRef.current.contains(event.target as Node)) {
        setOpen(false)
      }
    }
    function handleKey(event: KeyboardEvent) {
      if (event.key === 'Escape') setOpen(false)
    }
    document.addEventListener('mousedown', handlePointer)
    document.addEventListener('keydown', handleKey)
    return () => {
      document.removeEventListener('mousedown', handlePointer)
      document.removeEventListener('keydown', handleKey)
    }
  }, [open])

  return (
    <div
      ref={containerRef}
      className={cn(
        'relative flex flex-wrap items-center gap-x-2 gap-y-1.5',
        className,
      )}
      data-testid={testId}
    >
      {chips.length === 0 ? (
        <span className="text-ink-faint font-mono text-[10.5px] uppercase tracking-[0.08em]">
          {copy.emptyHint}
        </span>
      ) : (
        chips.map((chip) => (
          <span
            key={chip.id}
            className="border-border-light text-ink-muted inline-flex items-center gap-1.5 rounded-paper border bg-page px-2 py-0.5"
            data-testid={
              testId
                ? `${testId}-chip-${chip.id}`
                : `paper-filter-chip-${chip.id}`
            }
          >
            <span className="font-mono text-[10px] uppercase tracking-[0.08em] text-ink-faint">
              {chip.label}
            </span>
            <span className="font-sans text-[12px]">{chip.value}</span>
            <button
              type="button"
              className="text-ink-faint hover:text-ink focus-visible:text-ink ml-0.5 leading-none transition-colors"
              aria-label={copy.removeFilterAria
                .replace('{label}', chip.label)
                .replace('{value}', chip.value)}
              onClick={() => onRemove(chip.id)}
            >
              ×
            </button>
          </span>
        ))
      )}
      <button
        type="button"
        onClick={() => setOpen((value) => !value)}
        aria-expanded={open}
        aria-haspopup="dialog"
        className={cn(
          'border-border-light text-ink-muted hover:text-ink hover:border-ink-muted rounded-paper border border-dashed px-2.5 py-0.5 font-mono text-[10.5px] uppercase tracking-[0.08em] transition-colors',
          open && 'border-ink-muted text-ink',
        )}
        data-testid={testId ? `${testId}-add` : 'paper-filter-add'}
      >
        {copy.addFilter}
      </button>
      {chips.length > 0 ? (
        <button
          type="button"
          onClick={onClearAll}
          className="text-ink-faint hover:text-ink font-mono text-[10.5px] uppercase tracking-[0.08em] underline-offset-2 transition-colors hover:underline"
          data-testid={
            testId ? `${testId}-clear-all` : 'paper-filter-clear-all'
          }
        >
          {copy.clearAll}
        </button>
      ) : null}

      {open ? (
        <div
          role="dialog"
          aria-label={copy.popoverTitle}
          // z-50 wins over the sticky `PaperContactSheet` toolbar
          // (`top-0 z-[11]`) so the popover sits above the day-nav pill
          // instead of being clipped by it. max-w-[calc(100vw-2rem)]
          // keeps the form inside the viewport on narrow windows.
          className="border-border-default rounded-paper absolute left-0 top-full z-50 mt-2 w-[320px] max-w-[calc(100vw-2rem)] border bg-paper p-4 shadow-paper-soft"
          data-testid={testId ? `${testId}-popover` : 'paper-filter-popover'}
        >
          <div className="mb-3 flex items-center justify-between">
            <span className="font-mono text-[10.5px] uppercase tracking-[0.08em] text-ink-faint">
              {copy.popoverTitle}
            </span>
            <button
              type="button"
              aria-label={copy.closeLabel}
              onClick={() => setOpen(false)}
              className="text-ink-faint hover:text-ink leading-none transition-colors"
            >
              ×
            </button>
          </div>
          <form
            className="flex flex-col gap-3"
            onSubmit={(event) => {
              event.preventDefault()
              onApply(draft)
              setOpen(false)
            }}
          >
            <FilterField label={copy.fieldDomain}>
              <input
                type="text"
                value={draft.domain}
                onChange={(event) =>
                  setDraft((prev) => ({ ...prev, domain: event.target.value }))
                }
                className="border-border-light rounded-paper border bg-page px-2 py-1 font-mono text-[12px]"
                data-testid={
                  testId
                    ? `${testId}-input-domain`
                    : 'paper-filter-input-domain'
                }
              />
            </FilterField>
            {browserOptions.length > 1 ? (
              <FilterField label={copy.fieldBrowser}>
                <select
                  value={draft.browserKind}
                  onChange={(event) =>
                    setDraft((prev) => ({
                      ...prev,
                      browserKind: event.target.value,
                    }))
                  }
                  className="border-border-light rounded-paper border bg-page px-2 py-1 font-sans text-[12px]"
                  data-testid={
                    testId
                      ? `${testId}-input-browser`
                      : 'paper-filter-input-browser'
                  }
                >
                  <option value="">{copy.selectAllBrowsers}</option>
                  {browserOptions.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              </FilterField>
            ) : null}
            {profileOptions.length > 1 ? (
              <FilterField label={copy.fieldProfile}>
                <select
                  value={draft.profileId}
                  onChange={(event) =>
                    setDraft((prev) => ({
                      ...prev,
                      profileId: event.target.value,
                    }))
                  }
                  className="border-border-light rounded-paper border bg-page px-2 py-1 font-sans text-[12px]"
                  data-testid={
                    testId
                      ? `${testId}-input-profile`
                      : 'paper-filter-input-profile'
                  }
                >
                  <option value="">{copy.selectAllProfiles}</option>
                  {profileOptions.map((option) => (
                    <option key={option.value} value={option.value}>
                      {option.label}
                    </option>
                  ))}
                </select>
              </FilterField>
            ) : null}
            <div className="flex gap-2">
              <FilterField label={copy.fieldStart} className="flex-1">
                <input
                  type="date"
                  value={draft.start}
                  onChange={(event) =>
                    setDraft((prev) => ({ ...prev, start: event.target.value }))
                  }
                  className="border-border-light rounded-paper border bg-page px-2 py-1 font-mono text-[12px]"
                  data-testid={
                    testId
                      ? `${testId}-input-start`
                      : 'paper-filter-input-start'
                  }
                />
              </FilterField>
              <FilterField label={copy.fieldEnd} className="flex-1">
                <input
                  type="date"
                  value={draft.end}
                  onChange={(event) =>
                    setDraft((prev) => ({ ...prev, end: event.target.value }))
                  }
                  className="border-border-light rounded-paper border bg-page px-2 py-1 font-mono text-[12px]"
                  data-testid={
                    testId ? `${testId}-input-end` : 'paper-filter-input-end'
                  }
                />
              </FilterField>
            </div>
            <label className="flex items-center gap-2 font-mono text-[11px] uppercase tracking-[0.08em] text-ink-muted">
              <input
                type="checkbox"
                checked={draft.regexMode}
                onChange={(event) =>
                  setDraft((prev) => ({
                    ...prev,
                    regexMode: event.target.checked,
                  }))
                }
                data-testid={
                  testId ? `${testId}-input-regex` : 'paper-filter-input-regex'
                }
              />
              {copy.fieldRegex}
            </label>
            <button
              type="submit"
              className="bg-accent-strong text-accent-on-strong rounded-paper py-1.5 font-mono text-[11px] uppercase tracking-[0.08em] transition-opacity hover:opacity-90"
              data-testid={testId ? `${testId}-apply` : 'paper-filter-apply'}
            >
              {copy.applyLabel}
            </button>
          </form>
        </div>
      ) : null}
    </div>
  )
}

function FilterField({
  label,
  className,
  children,
}: {
  label: string
  className?: string
  children: React.ReactNode
}) {
  return (
    <label className={cn('flex flex-col gap-1', className)}>
      <span className="font-mono text-[10px] uppercase tracking-[0.08em] text-ink-faint">
        {label}
      </span>
      {children}
    </label>
  )
}
