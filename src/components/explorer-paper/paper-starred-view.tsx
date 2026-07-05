/**
 * PaperStarredView — the Starred hub.
 *
 * A focused Explorer mode (NOT a 4th nav item) that lists the user's starred
 * pages and sources, grouped Pages / Sources, sortable by recently-starred or
 * most-revisited. Pure presentation: the route loads `list_stars` and passes
 * the items + handlers in.
 *
 * ## Responsibilities
 * - Render the hub header (eyebrow + title + description + sort control).
 * - Render the Pages group as contact cards and the Sources group as rows,
 *   reusing the same `PaperContactFrame` renderer the Browse contact sheet uses.
 * - Render the loading skeleton and the empty state ("Star a page to keep it
 *   here.").
 *
 * ## Not responsible for
 * - Loading / sorting data — the route owns `list_stars` + the sort state.
 * - Star toggling within the hub — each card/row forwards `onToggle` to the
 *   route's optimistic star hook (un-starring removes the item on next load).
 *
 * ## Performance notes
 * - Renders only the already-bounded starred set the route fetched (hundreds at
 *   most). No archive scan happens here.
 */

import { cn } from '@/lib/cn'
import type { StarListItem, StarSort } from '@/lib/backend-client'
import {
  getDomainAbbr,
  getDomainColor,
} from '@/pages/explorer/paper/domain-color'
import { StarToggle } from '@/components/shell/star-toggle'
import { PaperContactFrame } from './paper-contact-frame'

export interface PaperStarredViewCopy {
  eyebrow: string
  title: string
  description: string
  groupPages: string
  groupSources: string
  sortLabel: string
  sortRecent: string
  sortRevisited: string
  loading: string
  emptyTitle: string
  emptyBody: string
  /** Empty-state CTA link copy, e.g. "Browse your history →". */
  emptyCta: string
  /** Visit-count chip template, e.g. "{count}×". */
  visitCountTemplate: string
  starAction: string
  unstarAction: string
  /** State words for the star's polite live region. */
  statusStarred: string
  statusUnstarred: string
}

export interface PaperStarredViewProps {
  items: readonly StarListItem[]
  loading: boolean
  sort: StarSort
  onSortChange: (sort: StarSort) => void
  /** Open a starred page in the detail panel / Browse. */
  onSelect?: (item: StarListItem) => void
  /** Toggle (un-star) a starred entity. */
  onToggleStar: (item: StarListItem) => void
  /** Leave the empty hub for the Browse surface (empty-state CTA). */
  onBrowseHistory?: () => void
  copy: PaperStarredViewCopy
  className?: string
  testId?: string
}

export function PaperStarredView({
  items,
  loading,
  sort,
  onSortChange,
  onSelect,
  onToggleStar,
  onBrowseHistory,
  copy,
  className,
  testId,
}: PaperStarredViewProps) {
  const pages = items.filter((item) => item.entityKind === 'url')
  const sources = items.filter((item) => item.entityKind === 'domain')
  const isEmpty = !loading && items.length === 0

  return (
    <section
      data-testid={testId ?? 'paper-starred-view'}
      className={cn('flex flex-col gap-6', className)}
    >
      <header className="flex flex-wrap items-end justify-between gap-3">
        <div className="min-w-0">
          <div className="text-ink-faint font-mono text-[9.5px] uppercase tracking-[0.1em]">
            {copy.eyebrow}
          </div>
          <h1 className="text-ink mt-1 font-serif text-[22px] leading-[1.2] tracking-[-0.01em]">
            {copy.title}
          </h1>
          <p className="text-ink-muted mt-1 font-serif text-[13px] italic">
            {copy.description}
          </p>
        </div>
        <label className="flex items-center gap-2">
          <span className="text-ink-faint font-mono text-[9.5px] uppercase tracking-[0.08em]">
            {copy.sortLabel}
          </span>
          <select
            value={sort}
            onChange={(event) => onSortChange(event.target.value as StarSort)}
            data-testid="paper-starred-sort"
            className={cn(
              'rounded-paper border-border-default bg-card-paper text-ink',
              'border px-2 py-1 font-sans text-[12px]',
              'focus:outline-none focus-visible:ring-1 focus-visible:ring-accent',
            )}
          >
            <option value="recently_starred">{copy.sortRecent}</option>
            <option value="most_revisited">{copy.sortRevisited}</option>
          </select>
        </label>
      </header>

      {loading ? (
        <StarredSkeleton label={copy.loading} />
      ) : isEmpty ? (
        <StarredEmpty
          title={copy.emptyTitle}
          body={copy.emptyBody}
          cta={copy.emptyCta}
          onBrowseHistory={onBrowseHistory}
        />
      ) : (
        <>
          {pages.length > 0 ? (
            <div>
              <GroupHeading label={copy.groupPages} count={pages.length} />
              <div className="grid grid-cols-[repeat(auto-fill,minmax(195px,1fr))] gap-4">
                {pages.map((item) => (
                  <PaperContactFrame
                    key={`url::${item.entityKey}`}
                    entry={{
                      id: item.entityKey,
                      title: item.title || item.entityKey,
                      domain: item.domain || item.entityKey,
                      url: item.entityKey,
                      time: starredVisitChip(copy.visitCountTemplate, item),
                    }}
                    domainColor={getDomainColor(item.domain || item.entityKey)}
                    domainAbbr={getDomainAbbr(item.domain || item.entityKey)}
                    onClick={onSelect ? () => onSelect(item) : undefined}
                    star={{
                      starred: true,
                      onToggle: () => onToggleStar(item),
                      starLabel: copy.starAction,
                      unstarLabel: copy.unstarAction,
                      statusLabel: {
                        starred: copy.statusStarred,
                        unstarred: copy.statusUnstarred,
                      },
                    }}
                    testId={`paper-starred-page-${item.entityKey}`}
                  />
                ))}
              </div>
            </div>
          ) : null}

          {sources.length > 0 ? (
            <div>
              <GroupHeading label={copy.groupSources} count={sources.length} />
              <div className="flex flex-wrap gap-2">
                {sources.map((item) => (
                  <SourceChip
                    key={`domain::${item.entityKey}`}
                    item={item}
                    visitTemplate={copy.visitCountTemplate}
                    starAction={copy.starAction}
                    unstarAction={copy.unstarAction}
                    statusStarred={copy.statusStarred}
                    statusUnstarred={copy.statusUnstarred}
                    onSelect={onSelect}
                    onToggleStar={onToggleStar}
                  />
                ))}
              </div>
            </div>
          ) : null}
        </>
      )}
    </section>
  )
}

function starredVisitChip(template: string, item: StarListItem): string {
  return template.replace('{count}', String(item.visitCount))
}

function GroupHeading({ label, count }: { label: string; count: number }) {
  return (
    <div className="text-ink-faint mb-2 flex items-center gap-2 font-mono text-[9.5px] uppercase tracking-[0.1em]">
      <span>{label}</span>
      <span className="text-ink-faint tabular-nums">{count}</span>
    </div>
  )
}

function SourceChip({
  item,
  visitTemplate,
  starAction,
  unstarAction,
  statusStarred,
  statusUnstarred,
  onSelect,
  onToggleStar,
}: {
  item: StarListItem
  visitTemplate: string
  starAction: string
  unstarAction: string
  statusStarred: string
  statusUnstarred: string
  onSelect?: (item: StarListItem) => void
  onToggleStar: (item: StarListItem) => void
}) {
  return (
    <div className="group border-border-light rounded-paper bg-card-paper flex items-center gap-2 border px-3 py-2">
      <button
        type="button"
        onClick={onSelect ? () => onSelect(item) : undefined}
        disabled={!onSelect}
        data-testid={`paper-starred-source-${item.entityKey}`}
        className="text-ink-secondary hover:text-accent font-mono text-[12px] transition-colors enabled:cursor-pointer disabled:cursor-default"
      >
        {item.entityKey}
        {item.visitCount > 0 ? (
          <span className="text-ink-faint ml-2">
            {visitTemplate.replace('{count}', String(item.visitCount))}
          </span>
        ) : null}
      </button>
      {/* Shared StarToggle — one star identity across every surface. The chip's
          stars are always-on so the (already-starred) row's un-star action is
          discoverable without a hover. */}
      <StarToggle
        starred
        onToggle={() => onToggleStar(item)}
        starLabel={starAction}
        unstarLabel={unstarAction}
        statusLabel={{ starred: statusStarred, unstarred: statusUnstarred }}
        alwaysVisible
        size={14}
        testId={`paper-starred-source-star-${item.entityKey}`}
        className="h-5 w-5"
      />
    </div>
  )
}

function StarredSkeleton({ label }: { label: string }) {
  return (
    <div data-testid="paper-starred-skeleton" aria-busy="true">
      <span className="sr-only">{label}</span>
      <div className="grid grid-cols-[repeat(auto-fill,minmax(195px,1fr))] gap-4">
        {Array.from({ length: 6 }).map((_, index) => (
          <div
            key={index}
            className="rounded-paper border-border-light bg-card-paper animate-pulse border"
          >
            <div className="bg-hover aspect-[1.91/1] w-full" />
            <div className="space-y-2 p-[10px]">
              <div className="bg-hover h-3 w-3/4 rounded" />
              <div className="bg-hover h-2 w-1/2 rounded" />
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}

function StarredEmpty({
  title,
  body,
  cta,
  onBrowseHistory,
}: {
  title: string
  body: string
  cta: string
  onBrowseHistory?: () => void
}) {
  return (
    <div
      data-testid="paper-starred-empty"
      className="border-border-light rounded-paper flex flex-col items-center gap-2 border border-dashed px-6 py-16 text-center"
    >
      <svg
        viewBox="0 0 24 24"
        width={28}
        height={28}
        fill="none"
        stroke="currentColor"
        strokeWidth={1.6}
        strokeLinejoin="round"
        className="text-ink-faint"
        aria-hidden="true"
      >
        <path d="M12 3.5l2.6 5.3 5.9.9-4.25 4.15 1 5.85L12 17.9l-5.25 2.65 1-5.85L3.5 9.7l5.9-.9z" />
      </svg>
      <div className="text-ink font-serif text-[15px]">{title}</div>
      <div className="text-ink-muted font-serif text-[13px] italic">{body}</div>
      {onBrowseHistory ? (
        <button
          type="button"
          onClick={onBrowseHistory}
          data-testid="paper-starred-empty-cta"
          className="text-accent hover:text-accent-text mt-2 font-mono text-[11px] uppercase tracking-[0.08em] underline-offset-2 transition-colors hover:underline"
        >
          {cta}
        </button>
      ) : null}
    </div>
  )
}
