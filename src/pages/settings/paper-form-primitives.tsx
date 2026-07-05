/**
 * Shared paper-redesign form primitives for the Settings sub-sections.
 *
 * ## Responsibilities
 * - Provide a single `Field` row with label + optional help text, a `Toggle`
 *   switch, and a `SegmentedControl` radiogroup for use across every
 *   Settings sub-section.
 * - Keep the visual contract aligned with the paper-redesign design package
 *   (see `docs/design/handoff/paper-redesign/project/pk-settings.jsx`).
 *
 * ## Not responsible for
 * - Section-level layout (each section composes Field + Toggle +
 *   SegmentedControl into its own card body).
 * - Data binding or backend transport (callers thread real handlers).
 *
 * ## Dependencies
 * - `cn` for class merging only — no i18n, no shell-data dependency.
 *
 * Splitting these out lets Settings sub-sections (appearance, link-previews,
 * future general / ai / applock / profiles / derived / remote / platform)
 * share one source of truth, which is what Phase 3 of the paper redesign
 * formalizes.
 */

import { cn } from '@/lib/cn'

export interface FieldProps {
  label: string
  help?: string
  children: React.ReactNode
}

export function Field({ label, help, children }: FieldProps) {
  return (
    <div className="border-border-light border-b py-3 first:pt-0 last:border-b-0 last:pb-0">
      <div className="text-ink-faint mb-2 font-mono text-[10px] tracking-[0.08em] uppercase">
        {label}
      </div>
      <div className="flex items-center justify-between gap-4">
        <div className="flex-1">
          {help ? (
            <p className="text-ink-muted m-0 mb-2 font-sans text-[12px]">
              {help}
            </p>
          ) : null}
          {children}
        </div>
      </div>
    </div>
  )
}

export interface ToggleProps {
  value: boolean
  onChange: (value: boolean) => void
  onLabel: string
  offLabel: string
  testId?: string
  /**
   * Renders the switch as inert (visibly dimmed, `aria-disabled`, no click).
   * Additive and defaulted to false so existing call sites are unaffected.
   * Used when a hard precondition is missing (e.g. no system keychain) so the
   * control reads honestly instead of accepting a click that snaps back.
   */
  disabled?: boolean
}

export function Toggle({
  value,
  onChange,
  onLabel,
  offLabel,
  testId,
  disabled = false,
}: ToggleProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={value}
      // `aria-disabled` (not the native `disabled` attribute) keeps the switch
      // focusable/announced as disabled to AT while leaving the click to the
      // consumer's handler — the owning section decides the honest no-op (e.g.
      // SecuritySection bails when no keychain is present). Mirrors the common
      // "disabled but still discoverable" a11y pattern.
      aria-disabled={disabled || undefined}
      onClick={() => onChange(!value)}
      data-testid={testId}
      className={cn(
        'border-border-default rounded-paper inline-flex items-center gap-3 border px-3 py-1.5 text-[12px] transition-colors',
        disabled
          ? 'text-ink-faint cursor-not-allowed opacity-60'
          : value
            ? 'border-accent bg-accent-soft text-accent-text'
            : 'text-ink-muted hover:border-ink-muted hover:bg-hover',
      )}
    >
      <span
        className={cn(
          'inline-block h-3 w-3 rounded-full',
          value ? 'bg-accent' : 'bg-ink-faint',
        )}
      />
      {value ? onLabel : offLabel}
    </button>
  )
}

export interface SegmentedControlOption<Id extends string> {
  id: Id
  label: string
  hint?: string
}

export interface SegmentedControlProps<Id extends string> {
  options: Array<SegmentedControlOption<Id>>
  value: Id
  onChange: (id: Id) => void
  /** Stack the radio buttons vertically (used by the link-previews eviction picker). */
  stacked?: boolean
  /**
   * Disables every option so the control reads as inert. Used when a
   * gating field (e.g. the master fetch toggle) is off — the picker
   * still renders so the user can see what's available, but no choice
   * fires until the gate flips back on.
   */
  disabled?: boolean
  testId?: string
}

export function SegmentedControl<Id extends string>({
  options,
  value,
  onChange,
  stacked = false,
  disabled = false,
  testId,
}: SegmentedControlProps<Id>) {
  return (
    <div
      className={cn(
        'flex gap-2',
        stacked ? 'flex-col items-stretch' : 'flex-row items-center',
      )}
      role="radiogroup"
      data-testid={testId}
    >
      {options.map((option) => (
        <button
          key={option.id}
          type="button"
          role="radio"
          aria-checked={option.id === value}
          disabled={disabled}
          onClick={() => onChange(option.id)}
          data-testid={testId ? `${testId}-${option.id}` : undefined}
          className={cn(
            'border-border-default rounded-paper flex items-start gap-2 border px-3 py-2 text-left transition-colors',
            option.id === value
              ? 'border-accent bg-accent-soft text-accent-text'
              : 'text-ink hover:border-ink-muted hover:bg-hover',
            disabled &&
              'cursor-not-allowed opacity-60 hover:border-border-default hover:bg-transparent',
          )}
        >
          <span className="flex flex-col">
            <span className="font-sans text-[12.5px] font-medium">
              {option.label}
            </span>
            {option.hint ? (
              <span className="text-ink-faint mt-0.5 font-mono text-[10px]">
                {option.hint}
              </span>
            ) : null}
          </span>
        </button>
      ))}
    </div>
  )
}
