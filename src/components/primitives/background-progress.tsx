/**
 * Background progress strip — a non-blocking status footer for long-running
 * but user-deferrable shell actions like the manual backup.
 *
 * ## Responsibilities
 * - Render the busy-overlay payload as a thin bottom-edge strip with a
 *   determinate or indeterminate progress bar, the action label, optional
 *   detail / progress-value text, and an optional inline log line.
 * - Stay above the page content (sticky bottom, layered above the regular
 *   status bar) but never block input or scroll — the rest of the UI
 *   stays interactive while the action runs.
 *
 * ## Not responsible for
 * - Driving the progress events (the shell action does that and feeds us
 *   the same BusyOverlayState payload it would have given BusyOverlay).
 * - Deciding which actions are background vs blocking — that's the
 *   `BusyOverlayState.background` flag, set at the action site.
 */

import type { BusyOverlayState } from '@/app/shell-data-context'
import { cn } from '@/lib/cn'

export interface BackgroundProgressProps {
  state: BusyOverlayState
  fallbackLabel?: string
  className?: string
}

export function BackgroundProgress({
  state,
  fallbackLabel,
  className,
}: BackgroundProgressProps) {
  const label = state.label || fallbackLabel || ''
  const detail = state.detail ?? null
  const progress =
    typeof state.progressValue === 'number' && Number.isFinite(state.progressValue)
      ? Math.max(0, Math.min(100, Math.round(state.progressValue)))
      : null
  const log = state.logLines?.[state.logLines.length - 1] ?? null

  return (
    <div
      role="status"
      aria-live="polite"
      data-testid="background-progress"
      className={cn(
        'pointer-events-auto fixed inset-x-0 bottom-[32px] z-[20]',
        'flex flex-col gap-1 border-t border-border-light bg-paper/95 px-4 py-2',
        'backdrop-blur supports-[backdrop-filter]:bg-paper/85',
        className,
      )}
    >
      <div className="flex items-center gap-3 text-[11.5px] text-ink">
        <span
          aria-hidden="true"
          className="inline-block h-1.5 w-1.5 shrink-0 animate-pulse rounded-full bg-accent"
        />
        <span className="font-sans font-medium">{label}</span>
        {state.progressLabel ? (
          <span className="font-mono text-[10.5px] text-ink-faint">
            {state.progressLabel}
          </span>
        ) : null}
        {progress !== null ? (
          <span className="ml-auto font-mono text-[10.5px] tabular-nums text-ink-secondary">
            {progress}%
          </span>
        ) : null}
      </div>
      <div
        className={cn(
          'h-[3px] w-full overflow-hidden rounded-[2px] bg-border-light',
          progress === null ? 'relative' : null,
        )}
        aria-hidden="true"
      >
        <span
          className={cn(
            'block h-full bg-accent transition-[width] duration-200 ease-out',
            progress === null ? 'animate-[background-progress-glide_1.4s_ease-in-out_infinite] w-1/3' : null,
          )}
          style={progress === null ? undefined : { width: `${Math.max(progress, 2)}%` }}
        />
      </div>
      {detail || log ? (
        <div className="flex items-baseline gap-2 text-[10.5px] text-ink-faint">
          {detail ? <span className="truncate">{detail}</span> : null}
          {log && log !== detail ? (
            <span className="truncate font-mono">{log}</span>
          ) : null}
        </div>
      ) : null}
    </div>
  )
}
