/**
 * @file ai-gpu-section.tsx
 * @description GPU heavy-tier opt-in + re-embed trigger with cost estimate (W-AI-9 Sub-block D).
 * @module pages/settings
 *
 * ## Responsibilities
 * - Render an honest GPU-acceleration toggle: the draft `gpuEnabled` flips via a
 *   route-owned handler, but the checkbox is DISABLED (non-actuating, never a
 *   settable green) when the backend reports `gpuAvailable: false` — a CPU-only
 *   build cannot run Metal, so the control must not let a filled box assert "ON"
 *   for a build that can't honor it, and the copy says so.
 * - Offer two re-embed actions (working set, full archive), each showing the
 *   cost/time estimate BEFORE firing; the full-archive action is gated on
 *   gpuEnabled + gpuAvailable (it is impractical on the CPU).
 * - Surface live progress via bounded polling of the AI queue (the REACH-B
 *   queued/running pattern), so a long re-embed never freezes the UI.
 *
 * ## Not responsible for
 * - Persisting the `gpuEnabled` draft (the shared AI config Save owns that — the
 *   toggle mutates the draft only, like every other AI sub-toggle).
 * - The estimate math or scope semantics (those live in the Rust backend; this
 *   only displays the returned `ReembedEstimate` and fires `buildAiIndex`).
 *
 * ## Dependencies
 * - `backend.estimateReembed` / `backend.buildAiIndex` / `backend.loadAiQueueStatus`.
 *
 * ## Performance notes
 * - Tucked in a zero-JS `<details>` disclosure (secondary to the main AI config).
 * - Estimates are fetched lazily (only when the disclosure is open) and progress
 *   polling is bounded + cleared on unmount, so nothing runs on the render path or
 *   leaks a timer.
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import { backend } from '../../lib/backend-client'
import { useI18n } from '../../lib/i18n'
import type { AiSettings, ReembedEstimate, ReembedScope } from '../../lib/types'
import { cn } from '@/lib/cn'
import { Button } from '@/components/ui/button'

/**
 * Route-owned slice this section binds to.
 */
export interface AiGpuSectionProps {
  /** The live AI draft (the GPU toggle reads `gpuEnabled` off it). */
  settings: AiSettings | null
  /** Inert when AI is off or a save is in flight (matches the other AI editors). */
  disabled: boolean
  /** Flips the draft `gpuEnabled` (persisted by the shared AI config Save). */
  onToggleGpu: () => void
}

/** Bounded progress-poll cadence + ceiling (REACH-B): never an unbounded loop. */
const POLL_INTERVAL_MS = 1500
const MAX_POLLS = 80

type RunState =
  | { kind: 'idle' }
  | { kind: 'queued' }
  | { kind: 'running'; queued: number; running: number }
  | { kind: 'done' }
  // Hit the poll ceiling while work was still draining: we can't honestly claim
  // "complete" (a working-set re-embed can run for hours on CPU), so we settle to
  // a neutral "still running in the background — check Jobs" state instead.
  | { kind: 'background' }
  | { kind: 'error' }

/**
 * Renders the collapsible GPU + re-embed controls. Returns null before a draft
 * exists (the parent guards this, but the local guard keeps it honest standalone).
 */
export function AiGpuSection({
  settings,
  disabled,
  onToggleGpu,
}: AiGpuSectionProps) {
  const { language, t } = useI18n()
  const [open, setOpen] = useState(false)
  const [estimates, setEstimates] = useState<
    Partial<Record<ReembedScope, ReembedEstimate>>
  >({})
  const [estimateError, setEstimateError] = useState(false)
  const [runState, setRunState] = useState<RunState>({ kind: 'idle' })
  const [busyScope, setBusyScope] = useState<ReembedScope | null>(null)
  const pollTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const pollCount = useRef(0)

  // The backend's honest source of truth for whether THIS build can run Metal.
  // `gpuAvailable` is a build-level flag (`cfg!(feature = "metal")`), identical on
  // both estimates (they are fetched together), so reading it off the working-set
  // estimate is sufficient. It defaults to false until an estimate resolves, so the
  // UI never optimistically claims GPU before the backend confirms it.
  const gpuAvailable = estimates['working-set']?.gpuAvailable ?? false
  const gpuEnabled = settings?.gpuEnabled ?? false
  // M-3 consent gate at the firing site: a re-embed enqueues an embedding job (provider egress + a
  // large derived-vector tail), so it requires the semantic-index (Smart search) sub-flag, not just
  // the master AI switch. When Smart search is off we disable BOTH re-embed actions and show an
  // honest, actionable reason instead of a dead button — mirroring the backend gate in
  // `build_ai_index_now`, so the UI never offers an action the backend will refuse.
  const semanticIndexEnabled = settings?.semanticIndexEnabled ?? false

  const clearPoll = useCallback(() => {
    if (pollTimer.current !== null) {
      clearTimeout(pollTimer.current)
      pollTimer.current = null
    }
  }, [])

  // Clearing the poll timer on unmount is the one race guard that matters: it
  // stops any scheduled poll from re-firing, so the loop never outlives the
  // component (a single in-flight resolution settling state is harmless).
  useEffect(() => clearPoll, [clearPoll])

  // Fetch both estimates when the disclosure opens (lazy — nothing runs while
  // collapsed). A failure degrades into an honest "could not load" note rather
  // than throwing, so one backend hiccup never breaks the section. The `cancelled`
  // flag drops a stale resolution if the disclosure closes/reopens before it lands.
  useEffect(() => {
    if (!open) {
      return
    }
    let cancelled = false
    setEstimateError(false)
    Promise.all([
      backend.estimateReembed('working-set'),
      backend.estimateReembed('full'),
    ])
      .then(([workingSet, full]) => {
        if (!cancelled) {
          setEstimates({ 'working-set': workingSet, full })
        }
      })
      .catch(() => {
        if (!cancelled) {
          setEstimateError(true)
        }
      })
    return () => {
      cancelled = true
    }
  }, [open])

  // Bounded queue poll: re-reads queued/running until the queue genuinely drains
  // (→ "done") or the poll ceiling is hit while work is still pending. A working-set
  // re-embed can run for hours on CPU, so we MUST NOT fabricate "complete" at the
  // ceiling — we settle to an honest "still running in the background, check Jobs"
  // state. Always off the render path + the timer is cleared on unmount, so a long
  // re-embed never freezes the UI or leaks a loop.
  const pollQueue = useCallback(() => {
    backend
      .loadAiQueueStatus()
      .then((status) => {
        const { queued, running } = status
        if (queued + running === 0) {
          // The queue genuinely drained — this is the only state that can honestly
          // claim the re-embed finished.
          setRunState({ kind: 'done' })
          return
        }
        if (pollCount.current >= MAX_POLLS) {
          // Ceiling reached but work is still pending: don't lie that it's done.
          setRunState({ kind: 'background' })
          return
        }
        setRunState({ kind: 'running', queued, running })
        pollCount.current += 1
        pollTimer.current = setTimeout(pollQueue, POLL_INTERVAL_MS)
      })
      .catch(() => {
        // A status hiccup ends the visible progress at the honest "running in the
        // background" state rather than spinning forever or claiming completion;
        // the queue keeps draining in the worker regardless.
        setRunState({ kind: 'background' })
      })
  }, [])

  const startReembed = useCallback(
    async (scope: ReembedScope) => {
      setBusyScope(scope)
      setRunState({ kind: 'queued' })
      clearPoll()
      pollCount.current = 0
      try {
        await backend.buildAiIndex({
          fullRebuild: scope === 'full',
          clearOnly: false,
          scope,
        })
        pollTimer.current = setTimeout(pollQueue, POLL_INTERVAL_MS)
      } catch {
        setRunState({ kind: 'error' })
      } finally {
        setBusyScope(null)
      }
    },
    [clearPoll, pollQueue],
  )

  if (!settings) {
    return null
  }

  const running = runState.kind === 'queued' || runState.kind === 'running'
  const fullAvailable = gpuEnabled && gpuAvailable

  return (
    <details
      className="border-border-light rounded-paper border"
      data-testid="ai-gpu-section"
      onToggle={(event) => setOpen(event.currentTarget.open)}
    >
      <summary
        className="text-ink-faint hover:text-ink-muted cursor-pointer list-none px-3 py-2.5 font-mono text-[10px] tracking-[0.08em] uppercase select-none"
        data-testid="ai-gpu-summary"
      >
        {t('settings.aiGpuTitle')}
      </summary>
      <div className="border-border-light flex flex-col gap-4 border-t px-3 py-3">
        <p className="text-ink-muted m-0 font-sans text-[12px] leading-[1.55]">
          {t('settings.aiGpuIntro')}
        </p>

        {/* GPU toggle — honest about whether this build can actually use it. */}
        <div className={cn('flex flex-col gap-1.5', disabled && 'opacity-60')}>
          <label className="toggleRow">
            <span className="flex items-center gap-2">
              {t('settings.aiGpuToggleLabel')}
              <span
                className="text-ink-faint font-mono text-[10px] tracking-[0.04em] uppercase"
                data-testid="ai-gpu-build-badge"
              >
                {gpuAvailable
                  ? t('settings.aiGpuAvailableBadge')
                  : t('settings.aiGpuUnavailableBadge')}
              </span>
            </span>
            <input
              aria-describedby="ai-gpu-help"
              checked={gpuEnabled}
              disabled={disabled || !gpuAvailable}
              type="checkbox"
              onChange={onToggleGpu}
              data-testid="ai-gpu-toggle"
            />
          </label>
          <div
            className="text-ink-muted m-0 flex flex-col gap-1 font-sans text-[12px] leading-[1.5]"
            id="ai-gpu-help"
          >
            <p className="m-0">{t('settings.aiGpuToggleHelp')}</p>
            {!gpuAvailable ? (
              <p
                className="text-ink-faint m-0 italic"
                data-testid="ai-gpu-unavailable"
              >
                {t('settings.aiGpuUnavailable')}
              </p>
            ) : null}
          </div>
        </div>

        {/* Re-embed actions, each with the cost estimate shown before firing. */}
        <div className="flex flex-col gap-3" data-testid="ai-reembed">
          <span className="text-ink font-sans text-[12.5px] font-medium">
            {t('settings.aiReembedTitle')}
          </span>
          {estimateError ? (
            <p
              className="text-ink-faint m-0 font-sans text-[12px] italic"
              data-testid="ai-reembed-estimate-error"
            >
              {t('settings.aiReembedEstimateError')}
            </p>
          ) : null}

          <ReembedAction
            scope="working-set"
            label={t('settings.aiReembedWorkingSetLabel')}
            help={t('settings.aiReembedWorkingSetHelp')}
            estimate={estimates['working-set']}
            actionDisabled={
              disabled || running || busyScope !== null || !semanticIndexEnabled
            }
            blockedReason={
              semanticIndexEnabled
                ? null
                : t('settings.aiReembedRequiresSemanticIndex')
            }
            language={language}
            t={t}
            onStart={() => startReembed('working-set')}
          />
          <ReembedAction
            scope="full"
            label={t('settings.aiReembedFullLabel')}
            help={t('settings.aiReembedFullHelp')}
            estimate={estimates.full}
            actionDisabled={
              disabled ||
              running ||
              busyScope !== null ||
              !semanticIndexEnabled ||
              !fullAvailable
            }
            blockedReason={
              !semanticIndexEnabled
                ? t('settings.aiReembedRequiresSemanticIndex')
                : fullAvailable
                  ? null
                  : t('settings.aiReembedFullRequiresGpu')
            }
            language={language}
            t={t}
            onStart={() => startReembed('full')}
          />

          {/* Live, bounded progress — never blocks the main thread. */}
          <ReembedStatus runState={runState} t={t} />
        </div>
      </div>
    </details>
  )
}

interface ReembedActionProps {
  scope: ReembedScope
  label: string
  help: string
  estimate: ReembedEstimate | undefined
  actionDisabled: boolean
  /** When non-null, the action is unavailable and this honest reason is shown instead of the button. */
  blockedReason: string | null
  language: string
  t: (key: string) => string
  onStart: () => void
}

/**
 * One re-embed action: label + help, the cost estimate (pages + CPU/GPU minutes),
 * and a Start button — or, when blocked (e.g. full archive without GPU), an honest
 * reason in place of the button rather than a dead control.
 */
function ReembedAction({
  scope,
  label,
  help,
  estimate,
  actionDisabled,
  blockedReason,
  language,
  t,
  onStart,
}: ReembedActionProps) {
  return (
    <div
      className="border-border-light rounded-paper flex flex-col gap-1.5 border p-2.5"
      data-testid={`ai-reembed-${scope}`}
    >
      <div className="flex items-baseline justify-between gap-3">
        <span className="text-ink font-sans text-[12.5px] font-medium">
          {label}
        </span>
        <ReembedEstimateChip
          estimate={estimate}
          language={language}
          t={t}
          scope={scope}
        />
      </div>
      <p className="text-ink-muted m-0 font-sans text-[12px] leading-[1.5]">
        {help}
      </p>
      {blockedReason ? (
        <p
          className="text-ink-faint m-0 font-sans text-[12px] leading-[1.5] italic"
          data-testid={`ai-reembed-${scope}-blocked`}
        >
          {blockedReason}
        </p>
      ) : (
        <Button
          type="button"
          variant="outline"
          className="self-start"
          disabled={actionDisabled}
          onClick={onStart}
          data-testid={`ai-reembed-${scope}-start`}
        >
          {t('settings.aiReembedStart')}
        </Button>
      )}
    </div>
  )
}

/**
 * The cost-estimate chip: page count + CPU minutes, and the GPU estimate when this
 * build can run Metal (otherwise an honest "needs a Metal build" note). Shows a
 * loading placeholder until the estimate resolves.
 */
function ReembedEstimateChip({
  estimate,
  language,
  t,
  scope,
}: {
  estimate: ReembedEstimate | undefined
  language: string
  t: (key: string) => string
  scope: ReembedScope
}) {
  if (!estimate) {
    return (
      <span
        className="text-ink-faint font-mono text-[11px]"
        data-testid={`ai-reembed-${scope}-estimate-loading`}
      >
        {t('settings.aiReembedEstimateLoading')}
      </span>
    )
  }
  const formatMinutes = (minutes: number) =>
    minutes.toLocaleString(language, {
      maximumFractionDigits: minutes < 10 ? 1 : 0,
    })
  const pages = estimate.pageCount.toLocaleString(language)
  return (
    <span
      className="text-ink-muted flex flex-col items-end font-mono text-[11px] tabular-nums"
      data-testid={`ai-reembed-${scope}-estimate`}
    >
      <span>
        {t('settings.aiReembedEstimatePages').replace('{count}', pages)}
      </span>
      <span>
        {t('settings.aiReembedEstimateCpu').replace(
          '{minutes}',
          formatMinutes(estimate.estMinutesCpu),
        )}
      </span>
      <span className="text-ink-faint">
        {estimate.gpuAvailable
          ? t('settings.aiReembedEstimateGpu').replace(
              '{minutes}',
              formatMinutes(estimate.estMinutesGpu),
            )
          : t('settings.aiReembedEstimateGpuUnavailable')}
      </span>
    </span>
  )
}

/**
 * Live re-embed status with an sr-only live region so a screen reader hears the
 * queued → running → done milestones without per-tick spam.
 */
function ReembedStatus({
  runState,
  t,
}: {
  runState: RunState
  t: (key: string) => string
}) {
  let message: string | null = null
  let testid = 'ai-reembed-status'
  switch (runState.kind) {
    case 'queued':
      message = t('settings.aiReembedQueued')
      break
    case 'running':
      message = t('settings.aiReembedProgress')
        .replace('{queued}', String(runState.queued))
        .replace('{running}', String(runState.running))
      break
    case 'done':
      message = t('settings.aiReembedDone')
      break
    case 'background':
      message = t('settings.aiReembedBackground')
      break
    case 'error':
      message = t('settings.aiReembedError')
      testid = 'ai-reembed-error'
      break
    default:
      message = null
  }
  if (message === null) {
    return null
  }
  return (
    <p
      aria-live="polite"
      className="text-ink-muted m-0 font-sans text-[12px] leading-[1.5]"
      data-testid={testid}
    >
      {message}
    </p>
  )
}
