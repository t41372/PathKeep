/**
 * PaperEnrichedContent — the detail-panel "Enriched content" section
 * (W-ENRICH-1, 06 §6).
 *
 * Renders the structured content PathKeep already fetched for the open page —
 * a GitHub repo's description + topic chips, or a generic page summary — plus a
 * manual "Fetch now" PME button. It is purely presentational: a hook owns the
 * `list_visit_enrichment` read and the `content_fetch_now` write, mirroring the
 * `StarToggle` (presentational) + `useDesktopStars` (hook) split.
 *
 * ## Responsibilities
 * - Render exactly one honest state at a time: loading, disabled (consent off),
 *   error, empty (never fetched / nothing readable), an honest failure status
 *   (login wall / non-HTML / blocked / rate-limited), or the enriched body.
 * - Render the source label + fetched-at, GitHub topic chips, and the summary.
 * - Offer "Fetch now"; disable it (with an explanation) when consent is off, and
 *   show a "fetching" state after a successful enqueue.
 *
 * ## Not responsible for
 * - Transport / data loading (the caller's hook supplies `state`).
 * - i18n resolution (the caller threads a typed `copy` bundle).
 *
 * ## Performance / fluidity
 * - The caller surfaces a skeleton within ~100 ms by passing `state.status =
 *   'loading'`; this component never awaits the backend on the render path. The
 *   section animates in (motion-safe) and the chips never reflow the panel.
 */

import { cn } from '@/lib/cn'
import type { EnrichmentView } from './paper-enriched-content-helpers'

export interface PaperEnrichedContentCopy {
  heading: string
  loading: string
  empty: string
  disabled: string
  error: string
  /** Fetched-at template with `{when}` placeholder. */
  fetchedAt: string
  sourceGithub: string
  sourceGeneric: string
  sourceUnknown: string
  topicsLabel: string
  /** Honest per-status messages for a non-success fetch. */
  statusEmpty: string
  statusBlocked: string
  statusError: string
  statusLogin: string
  statusUnsupported: string
  statusRateLimited: string
  /** Fetch-now PME button. */
  fetchNowAction: string
  fetchNowFetching: string
  fetchNowQueued: string
  fetchNowDisabledHint: string
  fetchNowError: string
}

/** Discriminated load state the hook hands the component. */
export type PaperEnrichedContentState =
  | { status: 'loading' }
  | { status: 'disabled' }
  | { status: 'error' }
  | { status: 'empty' }
  | { status: 'ready'; view: EnrichmentView }

export interface PaperEnrichedContentProps {
  state: PaperEnrichedContentState
  copy: PaperEnrichedContentCopy
  /** Whether content fetching is enabled for this page (drives the Fetch-now CTA). */
  fetchEnabled: boolean
  /** Whether a manual fetch was just enqueued (shows the "fetching" affordance). */
  fetchPending: boolean
  /** Set when the last manual fetch failed to enqueue. */
  fetchError: boolean
  onFetchNow: () => void
  testId?: string
}

export function PaperEnrichedContent({
  state,
  copy,
  fetchEnabled,
  fetchPending,
  fetchError,
  onFetchNow,
  testId,
}: PaperEnrichedContentProps) {
  return (
    <section
      data-testid={testId}
      className="motion-safe:animate-[paper-detail-backdrop-in_180ms_ease-out]"
    >
      <h3 className="text-ink-muted m-0 mb-[10px] font-serif text-[12px] italic">
        {copy.heading}
      </h3>

      {state.status === 'loading' ? (
        <EnrichedSkeleton label={copy.loading} testId={testId} />
      ) : state.status === 'disabled' ? (
        <EnrichedNote
          tone="muted"
          testId={testId ? `${testId}-disabled` : undefined}
        >
          {copy.disabled}
        </EnrichedNote>
      ) : state.status === 'error' ? (
        <EnrichedNote
          tone="error"
          role="alert"
          testId={testId ? `${testId}-error` : undefined}
        >
          {copy.error}
        </EnrichedNote>
      ) : state.status === 'empty' ? (
        <EnrichedNote
          tone="muted"
          testId={testId ? `${testId}-empty` : undefined}
        >
          {copy.empty}
        </EnrichedNote>
      ) : (
        <EnrichedBody view={state.view} copy={copy} testId={testId} />
      )}

      {/*
        Fetch-now is a PME trigger: it never silently fetches. When consent is
        off the button is disabled and a one-line hint explains why, pointing at
        Settings. When on, a click enqueues and the button reflects the pending
        fetch so the user sees their action took.
      */}
      <div className="mt-3 flex flex-wrap items-center gap-2">
        <button
          type="button"
          onClick={onFetchNow}
          disabled={!fetchEnabled || fetchPending}
          data-testid={testId ? `${testId}-fetch-now` : undefined}
          title={!fetchEnabled ? copy.fetchNowDisabledHint : undefined}
          className={cn(
            'rounded-paper inline-flex items-center gap-[5px] border px-[10px] py-[5px]',
            'font-sans text-[11.5px] transition-colors duration-150',
            'border-border-default text-ink-secondary bg-card-paper',
            'hover:border-ink-muted hover:text-ink',
            'disabled:cursor-not-allowed disabled:opacity-40',
          )}
        >
          {fetchPending ? copy.fetchNowFetching : copy.fetchNowAction}
        </button>
        {!fetchEnabled ? (
          <span
            className="text-ink-faint font-mono text-[10px]"
            data-testid={testId ? `${testId}-fetch-disabled-hint` : undefined}
          >
            {copy.fetchNowDisabledHint}
          </span>
        ) : fetchPending ? (
          <span
            role="status"
            aria-live="polite"
            className="text-ink-faint font-mono text-[10px]"
            data-testid={testId ? `${testId}-fetch-queued` : undefined}
          >
            {copy.fetchNowQueued}
          </span>
        ) : fetchError ? (
          <span
            role="alert"
            className="text-error font-mono text-[10px]"
            data-testid={testId ? `${testId}-fetch-error` : undefined}
          >
            {copy.fetchNowError}
          </span>
        ) : null}
      </div>
    </section>
  )
}

function sourceLabel(view: EnrichmentView, copy: PaperEnrichedContentCopy) {
  if (view.sourceKind === 'github') return copy.sourceGithub
  if (view.sourceKind === 'generic') return copy.sourceGeneric
  return copy.sourceUnknown
}

function statusMessage(view: EnrichmentView, copy: PaperEnrichedContentCopy) {
  switch (view.statusKind) {
    case 'empty':
      return copy.statusEmpty
    case 'blocked':
      return copy.statusBlocked
    case 'login':
      return copy.statusLogin
    case 'unsupported':
      return copy.statusUnsupported
    case 'rate-limited':
      return copy.statusRateLimited
    default:
      return copy.statusError
  }
}

function EnrichedBody({
  view,
  copy,
  testId,
}: {
  view: EnrichmentView
  copy: PaperEnrichedContentCopy
  testId?: string
}) {
  return (
    <div
      className="flex flex-col gap-2"
      data-testid={testId ? `${testId}-body` : undefined}
    >
      <div className="flex items-center gap-2">
        <span className="border-border-default text-ink-secondary bg-card-paper rounded-pill inline-flex items-center border px-[8px] py-[2px] font-mono text-[9.5px] uppercase tracking-[0.06em]">
          {sourceLabel(view, copy)}
        </span>
        <span className="text-ink-faint font-mono text-[10px]">
          {copy.fetchedAt.replace('{when}', view.fetchedAt)}
        </span>
      </div>

      {view.ok ? (
        <>
          {view.title ? (
            <div className="text-ink font-serif text-[13.5px] leading-[1.35]">
              {view.title}
            </div>
          ) : null}
          {view.description ? (
            <p className="text-ink-secondary m-0 font-serif text-[12.5px] leading-[1.5]">
              {view.description}
            </p>
          ) : null}
          {view.summary && view.summary !== view.description ? (
            <p className="text-ink-muted m-0 font-serif text-[12.5px] leading-[1.5] italic">
              {view.summary}
            </p>
          ) : null}
          {view.topics.length > 0 ? (
            <div>
              <div className="text-ink-faint mb-1 font-mono text-[9.5px] font-medium uppercase tracking-[0.08em]">
                {copy.topicsLabel}
              </div>
              <div
                className="flex flex-wrap gap-[6px]"
                data-testid={testId ? `${testId}-topics` : undefined}
              >
                {view.topics.map((topic) => (
                  <span
                    key={topic}
                    className="border-border-default bg-card-paper text-ink-secondary rounded-pill inline-flex items-center border px-[9px] py-[3px] font-mono text-[10.5px] tracking-[0.01em]"
                  >
                    {topic}
                  </span>
                ))}
              </div>
            </div>
          ) : null}
        </>
      ) : (
        // A fetched-but-not-successful row: show the honest status instead of an
        // empty body so the panel never implies content exists when it doesn't.
        <EnrichedNote
          tone="muted"
          testId={testId ? `${testId}-status` : undefined}
        >
          {statusMessage(view, copy)}
        </EnrichedNote>
      )}
    </div>
  )
}

function EnrichedSkeleton({
  label,
  testId,
}: {
  label: string
  testId?: string
}) {
  return (
    <div
      role="status"
      aria-live="polite"
      aria-label={label}
      data-testid={testId ? `${testId}-skeleton` : undefined}
      className="flex flex-col gap-2"
    >
      <span className="sr-only">{label}</span>
      <div className="bg-border-light h-3 w-1/3 rounded-[2px] motion-safe:animate-pulse" />
      <div className="bg-border-light h-3 w-4/5 rounded-[2px] motion-safe:animate-pulse" />
      <div className="bg-border-light h-3 w-2/3 rounded-[2px] motion-safe:animate-pulse" />
    </div>
  )
}

function EnrichedNote({
  children,
  tone,
  role,
  testId,
}: {
  children: React.ReactNode
  tone: 'muted' | 'error'
  role?: 'alert'
  testId?: string
}) {
  return (
    <p
      role={role}
      data-testid={testId}
      className={cn(
        'm-0 font-serif text-[12.5px] leading-[1.5] italic',
        tone === 'error' ? 'text-error' : 'text-ink-faint',
      )}
    >
      {children}
    </p>
  )
}
