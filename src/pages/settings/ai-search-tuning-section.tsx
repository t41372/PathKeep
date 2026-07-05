/**
 * @file ai-search-tuning-section.tsx
 * @description Power-user disclosure for the hybrid-search tuning knobs (W-AI-9 / W-AI-6).
 * @module pages/settings
 *
 * ## Responsibilities
 * - Render one labeled, bounded control per hybrid-search knob (RRF `k`, lexical
 *   vs semantic weight, starred boost), each with a slider + a number input that
 *   stay in sync, bound to the AI draft via route-owned handlers.
 * - Explain each knob in plain language (no jargon dump), including that the
 *   starred boost is BOUNDED so favorites rank higher without becoming a bookmark
 *   list.
 * - Offer a "Reset to defaults" affordance (60 / 1.0 / 1.0 / 0.15).
 *
 * ## Not responsible for
 * - Persisting the draft (the existing AI config Save owns that — these controls
 *   mutate the draft only, never auto-save).
 * - Clamp policy (lives in `search-tuning-helpers.ts`, mirroring the backend).
 *
 * ## Dependencies
 * - `search-tuning-helpers` for bounds/defaults; route handlers for the mutation.
 *
 * ## Performance notes
 * - Tucked inside a zero-JS `<details>` disclosure so it stays secondary and adds
 *   nothing to the render path until a power user opens it. Each edit is O(1).
 */

import { useI18n } from '../../lib/i18n'
import { cn } from '@/lib/cn'
import type { AiSettings } from '../../lib/types'
import {
  SEARCH_TUNING_BOUNDS,
  type SearchTuningKnob,
  resolveSearchTuningValue,
  searchTuningDiffersFromDefaults,
} from './search-tuning-helpers'

/**
 * Route-owned slice this disclosure binds to.
 */
export interface AiSearchTuningSectionProps {
  /** The live AI draft (resolved knob values are read off it). */
  settings: AiSettings | null
  /** Inert when AI is off or a save is in flight (matches the provider editors). */
  disabled: boolean
  /** Mutates one knob on the draft (clamped/sanitized by the route handler). */
  onChange: (knob: SearchTuningKnob, value: number) => void
  /** Restores all four knobs to their accepted defaults on the draft. */
  onReset: () => void
}

/**
 * One knob's display contract: its label, plain-language help, and the locale of
 * its formatted current value (integer for RRF `k`, fixed decimals for the rest).
 */
interface KnobView {
  knob: SearchTuningKnob
  label: string
  help: string
  /** Decimal places shown in the value chip + number input (0 for the integer k). */
  decimals: number
  testId: string
}

/**
 * Renders the collapsible advanced search-tuning controls. Returns null before a
 * draft exists (the parent AI section already guards this, but the local guard
 * keeps the control honest if it is ever mounted standalone).
 */
export function AiSearchTuningSection({
  settings,
  disabled,
  onChange,
  onReset,
}: AiSearchTuningSectionProps) {
  const { language, t } = useI18n()

  if (!settings) {
    return null
  }

  const knobs: KnobView[] = [
    {
      knob: 'hybridRrfK',
      label: t('settings.aiSearchTuningRrfKLabel'),
      help: t('settings.aiSearchTuningRrfKHelp'),
      decimals: 0,
      testId: 'ai-search-tuning-hybridRrfK',
    },
    {
      knob: 'lexicalWeight',
      label: t('settings.aiSearchTuningLexicalLabel'),
      help: t('settings.aiSearchTuningLexicalHelp'),
      decimals: 1,
      testId: 'ai-search-tuning-lexicalWeight',
    },
    {
      knob: 'semanticWeight',
      label: t('settings.aiSearchTuningSemanticLabel'),
      help: t('settings.aiSearchTuningSemanticHelp'),
      decimals: 1,
      testId: 'ai-search-tuning-semanticWeight',
    },
    {
      knob: 'starredBoost',
      label: t('settings.aiSearchTuningStarredLabel'),
      help: t('settings.aiSearchTuningStarredHelp'),
      decimals: 2,
      testId: 'ai-search-tuning-starredBoost',
    },
  ]

  const canReset = !disabled && searchTuningDiffersFromDefaults(settings)

  return (
    <details
      className="border-border-light rounded-paper border"
      data-testid="ai-search-tuning"
    >
      <summary
        className="text-ink-faint hover:text-ink-muted cursor-pointer list-none px-3 py-2.5 font-mono text-[10px] tracking-[0.08em] uppercase select-none"
        data-testid="ai-search-tuning-summary"
      >
        {t('settings.aiSearchTuningTitle')}
      </summary>
      <div className="border-border-light flex flex-col gap-4 border-t px-3 py-3">
        <p className="text-ink-muted m-0 font-sans text-[12px] leading-[1.55]">
          {t('settings.aiSearchTuningIntro')}
        </p>

        {knobs.map((view) => (
          <SearchTuningControl
            key={view.knob}
            view={view}
            value={resolveSearchTuningValue(settings, view.knob)}
            disabled={disabled}
            language={language}
            onChange={onChange}
          />
        ))}

        <div className="flex items-center gap-2">
          <button
            type="button"
            className="btn-secondary"
            disabled={!canReset}
            onClick={onReset}
            data-testid="ai-search-tuning-reset"
          >
            {t('settings.aiSearchTuningReset')}
          </button>
          <span className="text-ink-faint font-mono text-[10px]">
            {t('settings.aiSearchTuningResetHint')}
          </span>
        </div>
      </div>
    </details>
  )
}

interface SearchTuningControlProps {
  view: KnobView
  value: number
  disabled: boolean
  language: string
  onChange: (knob: SearchTuningKnob, value: number) => void
}

/**
 * One knob: label + value chip, plain-language help, and a slider/number-input
 * pair that both write the same clamped value through `onChange`. The number
 * input carries the same `min`/`max`/`step` as the slider so the browser's own
 * validation reinforces the bounds, and an emptied field parses to `NaN`, which
 * the route handler resets to the knob's default rather than persisting a hole.
 */
function SearchTuningControl({
  view,
  value,
  disabled,
  language,
  onChange,
}: SearchTuningControlProps) {
  const bounds = SEARCH_TUNING_BOUNDS[view.knob]
  const formattedValue = value.toLocaleString(language, {
    minimumFractionDigits: view.decimals,
    maximumFractionDigits: view.decimals,
  })

  return (
    <div
      className={cn('flex flex-col gap-1.5', disabled && 'opacity-60')}
      data-testid={view.testId}
    >
      <div className="flex items-baseline justify-between gap-3">
        <span className="text-ink font-sans text-[12.5px] font-medium">
          {view.label}
        </span>
        <span
          className="text-ink-muted font-mono text-[11px] tabular-nums"
          data-testid={`${view.testId}-value`}
        >
          {formattedValue}
        </span>
      </div>
      <p className="text-ink-muted m-0 font-sans text-[12px] leading-[1.5]">
        {view.help}
      </p>
      <div className="flex items-center gap-3">
        <input
          type="range"
          min={bounds.min}
          max={bounds.max}
          step={bounds.step}
          value={value}
          disabled={disabled}
          aria-label={view.label}
          onChange={(event) =>
            onChange(view.knob, event.currentTarget.valueAsNumber)
          }
          data-testid={`${view.testId}-slider`}
          className="flex-1 accent-[color:var(--accent)] disabled:cursor-not-allowed"
        />
        <input
          type="number"
          min={bounds.min}
          max={bounds.max}
          step={bounds.step}
          value={value}
          disabled={disabled}
          aria-label={view.label}
          onChange={(event) =>
            onChange(view.knob, event.currentTarget.valueAsNumber)
          }
          data-testid={`${view.testId}-input`}
          className={cn(
            'border-border-default rounded-paper bg-paper text-ink w-20 border px-2 py-1 text-right font-mono text-[11.5px] tabular-nums',
            'focus:border-accent focus:outline-none disabled:cursor-not-allowed',
          )}
        />
      </div>
    </div>
  )
}
