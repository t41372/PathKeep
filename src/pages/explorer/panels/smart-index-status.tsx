/**
 * In-surface Smart-search index status + Build CTA (REACH-B B1).
 *
 * Why this file exists:
 * - The Smart (relevance) search surface needs an honest, LIVE status for the
 *   semantic-index backfill that the Build CTA enqueues. `backend.buildAiIndex`
 *   only ENQUEUES a background job (the real embedding work runs for minutes on
 *   the worker queue), so a naive "flip to Building… then back" CTA would lie:
 *   it implies success on a bare enqueue while `indexedItems` is still ~0.
 * - This component reads the queue-derived `SmartIndexProgress` and renders the
 *   true phase — idle/empty, queued, running (with live queued/running counts),
 *   paused (with a resume affordance), or ready ("N pages indexed") — so the CTA
 *   never claims the index is built when it is only enqueued.
 *
 * Main declarations:
 * - `SmartIndexStatusCallout`
 *
 * Source-of-truth notes:
 * - Honesty grammar follows `docs/design/ux-principles.md` §2/§4 and
 *   `docs/design/ui-review-guardrails.md` §5 (long flows report phase, never a
 *   fake "Processing…"). The backend exposes no total-candidate count to the UI,
 *   so the running state shows the queue's own queued/running job counts via the
 *   shared `LoadingState progressLabel` pattern — never a fabricated percent.
 *
 * ## Responsibilities
 * - Map a `SmartIndexProgress` phase onto the right callout + CTA copy/state.
 * - Surface live, queue-reported progress while a build runs; point to resume
 *   when the queue is paused; show "N pages indexed" on completion.
 *
 * ## Not responsible for
 * - Deriving the progress (the route does that via `deriveSmartIndexProgress`).
 * - Polling the queue (the route owns the bounded poll).
 */

import { Link } from 'react-router-dom'
import { LoadingState } from '../../../components/primitives/loading-state'
import { StatusCallout } from '../../../components/primitives/status-callout'
import type { ResolvedLanguage } from '../../../lib/i18n'
import type { SmartIndexProgress } from '../paper-search-helpers'
import type { Translator } from '../types'

/**
 * Describes the props accepted by `SmartIndexStatusCallout`.
 */
export interface SmartIndexStatusCalloutProps {
  progress: SmartIndexProgress
  /** Honest title for the ready (built) state — the live AI status label. */
  readyTitle: string
  /** Whether an embedding provider is configured (build is impossible without). */
  hasEmbeddingProvider: boolean
  language: ResolvedLanguage
  explorerT: Translator
  /** Enqueue a (incremental) backfill. */
  onBuild: () => void
}

/**
 * Render the in-surface index status for the Smart-search relevance header.
 */
export function SmartIndexStatusCallout({
  progress,
  readyTitle,
  hasEmbeddingProvider,
  language,
  explorerT,
  onBuild,
}: SmartIndexStatusCalloutProps) {
  const { phase, indexedItems } = progress
  const indexEmpty = indexedItems === 0
  // Live, queue-reported counts — the only honest progress numbers the backend
  // surfaces (no total-candidate count exists), so we never fabricate a percent.
  const progressLabel = explorerT('queueProgressLabel', {
    queued: progress.queuedJobs.toLocaleString(language),
    running: progress.runningJobs.toLocaleString(language),
  })

  // Running: a backfill is actively embedding rows. Show the shared loading
  // progress pattern (phase + live queued/running counts, no fake percent) and a
  // disabled "Building…" CTA so the user cannot double-enqueue.
  if (phase === 'running') {
    return (
      <div
        className="flex flex-col gap-2"
        data-testid="explorer-smart-build-progress"
      >
        <StatusCallout
          tone="info"
          eyebrow={explorerT('smartIndexEyebrow')}
          title={explorerT('smartIndexBuildingTitle')}
          body={explorerT('smartIndexBuildingBody')}
          actions={
            <button
              className="btn-secondary"
              type="button"
              data-testid="explorer-smart-build-index"
              disabled
            >
              {explorerT('smartIndexBuildingCta')}
            </button>
          }
        />
        <LoadingState
          compact
          label={explorerT('smartIndexBuildingCta')}
          detail={explorerT('smartIndexBuildingDetail')}
          progressLabel={progressLabel}
        />
      </div>
    )
  }

  // Paused: enqueued but the queue is paused, so it will NOT progress until the
  // user resumes it. Say so honestly and point to resume — never imply the index
  // is building or built.
  if (phase === 'paused') {
    return (
      <StatusCallout
        tone="warning"
        eyebrow={explorerT('smartIndexEyebrow')}
        title={explorerT('smartIndexPausedTitle')}
        body={explorerT('smartIndexPausedBody')}
        actions={
          <Link
            className="btn-secondary"
            to="/settings#settings-ai"
            data-testid="explorer-smart-resume-index"
          >
            {explorerT('smartIndexResumeCta')}
          </Link>
        }
      />
    )
  }

  // Queued: enqueued and waiting for the worker to pick it up. The CTA stays
  // disabled (a build is pending) and we explicitly do NOT claim the index is
  // built — `indexedItems` may still be ~0.
  if (phase === 'queued') {
    return (
      <StatusCallout
        tone="info"
        eyebrow={explorerT('smartIndexEyebrow')}
        title={explorerT('smartIndexQueuedTitle')}
        body={explorerT('smartIndexQueuedBody')}
        actions={
          <button
            className="btn-secondary"
            type="button"
            data-testid="explorer-smart-build-index"
            disabled
          >
            {explorerT('smartIndexBuildingCta')}
          </button>
        }
      />
    )
  }

  // Idle: no backfill in flight. Either the index is empty ("nothing to rank
  // yet" → build) or it holds N pages (ready → optional rebuild). The CTA is the
  // only enabled-build entry point. `hasEmbeddingProvider` keeps this component
  // self-contained: a build can never be enqueued without a provider. (P1: the
  // ROUTE only mounts this when `smartAvailable`, which already implies a
  // provider, so the route-side guard was dead — the guard now lives here as a
  // genuine component-level invariant, exercised directly by the unit tests.)
  return (
    <StatusCallout
      tone={indexEmpty ? 'warning' : 'info'}
      eyebrow={explorerT('smartIndexEyebrow')}
      title={indexEmpty ? explorerT('smartIndexBuildTitle') : readyTitle}
      body={
        indexEmpty
          ? explorerT('smartIndexBuildBody')
          : explorerT('smartIndexReadyBody', {
              count: indexedItems.toLocaleString(language),
            })
      }
      actions={
        <button
          className="btn-secondary"
          type="button"
          data-testid="explorer-smart-build-index"
          onClick={onBuild}
          disabled={!hasEmbeddingProvider}
        >
          {explorerT('smartIndexBuildCta')}
        </button>
      }
    />
  )
}
