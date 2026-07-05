/**
 * @file ambient-task-bar.tsx
 * @description Fixed bottom "something is running" strip — the shell-level indicator for ANY
 *   long-running task (import, backup, AI index build, enrichment), and a click-through to the
 *   Activity page. Replaces the backup-only BackgroundProgress strip.
 *
 * ## Responsibilities
 * - Render one compact bottom-edge strip for the ambient task model: a single running task shows
 *   its label + progress; multiple concurrent tasks collapse into a "{count} tasks running" summary
 *   plus the primary task's label, so the chrome never stacks into multiple bars.
 * - Be a single click target that opens the Activity page — background work is deferrable, so the
 *   strip is an affordance, not a blocking modal.
 *
 * ## Not responsible for
 * - Selecting, ordering, or clamping tasks (see app/shell-ambient-tasks.ts).
 * - Localizing copy — it receives already-localized strings.
 * - Announcing progress to assistive tech: the Activity page owns milestone announcements, so this
 *   strip carries no aria-live and avoids screen-reader spam on every poll tick.
 */

import type { AmbientTasksModel } from '@/app/shell-ambient-tasks'
import { cn } from '@/lib/cn'

export interface AmbientTaskBarProps {
  model: AmbientTasksModel
  /** Navigate to the Activity page (the whole strip is the trigger). */
  onOpenActivity: () => void
  /** Localized "{count} tasks running" — shown only when count > 1. */
  summaryLabel: string
  /** Localized affordance/aria copy, e.g. "View background activity". */
  viewActivityLabel: string
}

export function AmbientTaskBar({
  model,
  onOpenActivity,
  summaryLabel,
  viewActivityLabel,
}: AmbientTaskBarProps) {
  const { count, primary } = model
  // Defensive: the shell only mounts this when count > 0, but a null primary means nothing to show.
  if (!primary) {
    return null
  }

  const progressValue = primary.progressValue
  const multiple = count > 1

  return (
    <button
      type="button"
      onClick={onOpenActivity}
      data-testid="ambient-task-bar"
      aria-label={`${multiple ? summaryLabel : primary.label}. ${viewActivityLabel}`}
      className={cn(
        'pointer-events-auto fixed inset-x-0 bottom-[32px] z-[20] flex w-full flex-col gap-1',
        'border-t border-border-light bg-paper/95 px-4 py-2 text-left backdrop-blur',
        'supports-[backdrop-filter]:bg-paper/85 cursor-pointer transition-colors hover:bg-paper',
      )}
    >
      <div className="flex items-center gap-3 text-[11.5px] text-ink">
        <span
          aria-hidden="true"
          className="inline-block h-1.5 w-1.5 shrink-0 animate-pulse rounded-full bg-accent"
        />
        {multiple ? (
          <>
            <span className="font-sans font-medium">{summaryLabel}</span>
            <span className="text-ink-faint">{primary.label}</span>
          </>
        ) : (
          <span className="font-sans font-medium">{primary.label}</span>
        )}
        {primary.progressLabel ? (
          <span className="font-mono text-[10.5px] text-ink-faint">
            {primary.progressLabel}
          </span>
        ) : null}
        {progressValue != null ? (
          <span className="ml-auto font-mono text-[10.5px] tabular-nums text-ink-secondary">
            {progressValue}%
          </span>
        ) : null}
      </div>
      <div
        className={cn(
          'h-[3px] w-full overflow-hidden rounded-[2px] bg-border-light',
          progressValue === null ? 'relative' : null,
        )}
        aria-hidden="true"
      >
        <span
          className={cn(
            'block h-full bg-accent transition-[width] duration-200 ease-out',
            progressValue === null ? 'pk-indeterminate-bar' : null,
          )}
          style={
            progressValue === null
              ? undefined
              : { width: `${Math.max(progressValue, 2)}%` }
          }
        />
      </div>
    </button>
  )
}
