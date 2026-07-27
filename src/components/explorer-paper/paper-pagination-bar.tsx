/**
 * PaperPaginationBar — the shared offset-pagination control for paper surfaces.
 *
 * `docs/features/recall.md` requires every long result list to carry a
 * pagination bar BOTH above and below the rows, each one showing
 * "current page / total pages" and offering first / previous / next / last,
 * a page-jump box, and a rows-per-page selector. The Browse contact sheet
 * already owned a footer-only variant of that chrome; this component is the
 * placement-agnostic version so a surface can mount the same affordances twice
 * without inventing a second visual dialect.
 *
 * ## Responsibilities
 * - Render the page readout + the six pagination affordances in the paper
 *   mono/rule visual language, in either a `top` or `bottom` placement.
 * - Derive the disabled state of each control from `page` / `pageCount` so a
 *   caller cannot navigate outside the result set by clicking.
 *
 * ## Not responsible for
 * - Owning page state — the caller (route URL state) is the single writer.
 * - Fetching, or knowing what a "row" is. It renders numbers it is handed.
 */

import { cn } from '@/lib/cn'

/**
 * Copy bundle for the bar. Carried inside the descriptor (rather than the
 * surface-wide copy object) so a surface can adopt pagination without widening
 * its own copy contract — the same pattern `PaperContactSheetPagination` uses.
 */
export interface PaperPaginationBarCopy {
  /** `aria-label` for the landmark, e.g. "Search results pages". */
  navLabel: string
  first: string
  previous: string
  next: string
  last: string
  /** Submit label for the page-jump box, e.g. "Go". */
  jump: string
  /** Accessible name for the page-jump number input. */
  pageInputLabel: string
  /** `{current}` / `{total}` — the mandatory "page N of M" readout. */
  pageSummary: string
  /** `{loaded}` / `{total}` — rows on this page vs. the whole match set. */
  resultsSummary: string
  pageSizeLabel: string
  /** `{count}` — one option label in the rows-per-page select. */
  pageSizeOption: string
}

/**
 * Everything the bar needs except where it is mounted. Surfaces hand ONE of
 * these to their view and the view renders it twice (top + bottom), so the two
 * bars can never drift out of sync.
 */
export interface PaperPaginationBarState {
  /** 1-based current page. */
  page: number
  /** Total pages in the result set (>= 1 whenever there are results). */
  pageCount: number
  /** Total matching rows across every page. */
  total: number
  /** Rows rendered on the current page. */
  loaded: number
  pageSize: number
  pageSizeOptions: readonly number[]
  /** Draft text of the page-jump box — owned by the caller so both bars share it. */
  pageInput: string
  onPageInputChange: (next: string) => void
  onFirst: () => void
  onPrevious: () => void
  onNext: () => void
  onLast: () => void
  onJump: () => void
  onChangePageSize: (next: number) => void
  copy: PaperPaginationBarCopy
}

export interface PaperPaginationBarProps extends PaperPaginationBarState {
  placement: 'top' | 'bottom'
  /** Required: the two placements need distinct hooks for tests + analytics. */
  testId: string
  className?: string
}

const buttonClass = cn(
  'border-border-default text-ink-muted rounded-paper border px-2.5 py-1',
  'transition-colors duration-150',
  'enabled:hover:border-ink-muted enabled:hover:text-ink enabled:cursor-pointer',
  'disabled:cursor-not-allowed disabled:opacity-50',
)

export function PaperPaginationBar({
  page,
  pageCount,
  total,
  loaded,
  pageSize,
  pageSizeOptions,
  pageInput,
  onPageInputChange,
  onFirst,
  onPrevious,
  onNext,
  onLast,
  onJump,
  onChangePageSize,
  copy,
  placement,
  testId,
  className,
}: PaperPaginationBarProps) {
  const atFirst = page <= 1
  const atLast = page >= pageCount

  return (
    <nav
      aria-label={copy.navLabel}
      data-testid={testId}
      className={cn(
        'border-border-light text-ink-muted flex flex-wrap items-center justify-between gap-3',
        'font-mono text-[10.5px] tracking-[0.04em]',
        placement === 'top' ? 'mb-4 border-b pb-3' : 'mt-6 border-t pt-4',
        className,
      )}
    >
      <div className="flex flex-col gap-0.5">
        <span data-testid={`${testId}-summary`}>
          {copy.pageSummary
            .replace('{current}', page.toLocaleString())
            .replace('{total}', pageCount.toLocaleString())}
        </span>
        <span className="text-ink-faint" data-testid={`${testId}-results`}>
          {copy.resultsSummary
            .replace('{loaded}', loaded.toLocaleString())
            .replace('{total}', total.toLocaleString())}
        </span>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <label className="flex items-center gap-1.5">
          <span className="text-ink-faint">{copy.pageSizeLabel}</span>
          <select
            value={pageSize}
            onChange={(event) =>
              onChangePageSize(Number.parseInt(event.target.value, 10))
            }
            data-testid={`${testId}-page-size`}
            className="border-border-default bg-card-paper text-ink rounded-paper border px-2 py-1 font-mono text-[10.5px]"
          >
            {pageSizeOptions.map((option) => (
              <option key={option} value={option}>
                {copy.pageSizeOption.replace('{count}', String(option))}
              </option>
            ))}
          </select>
        </label>

        <button
          type="button"
          onClick={onFirst}
          disabled={atFirst}
          data-testid={`${testId}-first`}
          className={buttonClass}
        >
          ⇤ {copy.first}
        </button>
        <button
          type="button"
          onClick={onPrevious}
          disabled={atFirst}
          data-testid={`${testId}-prev`}
          className={buttonClass}
        >
          ← {copy.previous}
        </button>

        {/* A real form so Enter inside the box jumps, matching the muscle
            memory of every other page box; the caller clamps out-of-range
            input rather than the browser silently swallowing it. */}
        <form
          className="flex items-center gap-1.5"
          data-testid={`${testId}-jump-form`}
          onSubmit={(event) => {
            event.preventDefault()
            onJump()
          }}
        >
          <input
            type="text"
            inputMode="numeric"
            value={pageInput}
            aria-label={copy.pageInputLabel}
            onChange={(event) => onPageInputChange(event.target.value)}
            data-testid={`${testId}-page-input`}
            className="border-border-default bg-card-paper text-ink rounded-paper w-12 border px-2 py-1 text-center font-mono text-[10.5px]"
          />
          <button type="submit" className={buttonClass}>
            {copy.jump}
          </button>
        </form>

        <button
          type="button"
          onClick={onNext}
          disabled={atLast}
          data-testid={`${testId}-next`}
          className={buttonClass}
        >
          {copy.next} →
        </button>
        <button
          type="button"
          onClick={onLast}
          disabled={atLast}
          data-testid={`${testId}-last`}
          className={buttonClass}
        >
          {copy.last} ⇥
        </button>
      </div>
    </nav>
  )
}
