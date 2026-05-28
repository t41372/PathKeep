/**
 * Horizontal step indicator used inside the paper Import wizard.
 *
 * Layout matches `pk-import.jsx` `.import-stepper`: numbered circles per
 * step, connecting lines between them, each step has one of three states
 * (idle / active / done). Active = accent fill; done = success fill +
 * checkmark glyph; idle = bordered transparent.
 *
 * ## Responsibilities
 * - Render the step circle + label per step, with a connecting line
 *   between each pair.
 *
 * ## Not responsible for
 * - Navigation logic — caller manages currentStep.
 */

import { cn } from '@/lib/cn'

export interface PaperImportStepperProps {
  steps: readonly string[]
  /** 0-based index of the currently-active step. */
  currentStep: number
  className?: string
  testId?: string
}

export function PaperImportStepper({
  steps,
  currentStep,
  className,
  testId,
}: PaperImportStepperProps) {
  return (
    <div
      data-testid={testId}
      className={cn(
        'border-border-light flex items-center gap-0 border-b px-6 py-[18px]',
        className,
      )}
    >
      {steps.map((label, index) => {
        const state =
          index < currentStep
            ? 'done'
            : index === currentStep
              ? 'active'
              : 'idle'
        return (
          <div
            key={label}
            className="flex items-center"
            data-step={state}
            data-testid={`paper-import-step-${index}`}
          >
            <div className="flex items-center gap-2">
              <div
                className={cn(
                  'grid h-6 w-6 shrink-0 place-items-center rounded-full border font-mono text-[11px]',
                  state === 'done' && 'bg-success border-success text-paper',
                  state === 'active' && 'bg-accent border-accent text-paper',
                  state === 'idle' &&
                    'bg-paper border-border-default text-ink-faint',
                )}
              >
                {state === 'done' ? '✓' : index + 1}
              </div>
              <span
                className={cn(
                  'font-serif text-[13px] tracking-[-0.005em]',
                  state === 'active' && 'text-ink font-medium',
                  state === 'done' && 'text-ink-secondary',
                  state === 'idle' && 'text-ink-faint',
                )}
              >
                {label}
              </span>
            </div>
            {index < steps.length - 1 ? (
              <div
                aria-hidden="true"
                className={cn(
                  'mx-[14px] h-px flex-1 min-w-[28px]',
                  index < currentStep ? 'bg-success' : 'bg-border-default',
                )}
              />
            ) : null}
          </div>
        )
      })}
    </div>
  )
}
