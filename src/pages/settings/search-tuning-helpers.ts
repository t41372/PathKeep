/**
 * @file search-tuning-helpers.ts
 * @description Pure clamp/reset helpers for the hybrid-search tuning knobs (W-AI-9 / W-AI-6).
 * @module pages/settings
 *
 * ## Responsibilities
 * - Mirror the backend `AiSettings::normalize_search_knobs` clamp bounds on the
 *   front end so a slider / number input can never push an out-of-range or NaN
 *   value into the AI draft (the backend re-clamps on load; the UI stays honest
 *   about the bounds it advertises).
 * - Apply one knob edit to an `AiSettings` draft, and reset all four knobs to
 *   their accepted defaults, without touching any other field.
 *
 * ## Not responsible for
 * - Rendering the controls (that is `ai-search-tuning-section.tsx`).
 * - Persisting the draft (the existing AI config Save owns that).
 *
 * ## Dependencies
 * - Only the `AiSettings` type. Pure data; side-effect free, so it unit-tests in
 *   isolation and is cheap to call on every keystroke.
 *
 * ## Performance notes
 * - O(1) per call; never clones the provider lists (a shallow spread keeps the
 *   knob edit off the render hot path).
 */

import type { AiSettings } from '../../lib/types'

/**
 * The four tunable knobs. Matched 1:1 to the Rust `AiSettings` fields (W-AI-6).
 */
export type SearchTuningKnob =
  | 'hybridRrfK'
  | 'lexicalWeight'
  | 'semanticWeight'
  | 'starredBoost'

/**
 * Accepted defaults for the knobs — the same values the backend `Default for
 * AiSettings` and the `default_*` fns use (RRF `k` = 60, equal 1.0 list weights,
 * a conservative 0.15 starred boost).
 */
export const SEARCH_TUNING_DEFAULTS: Record<SearchTuningKnob, number> = {
  hybridRrfK: 60,
  lexicalWeight: 1,
  semanticWeight: 1,
  starredBoost: 0.15,
}

/**
 * Inclusive bounds + a slider step for each knob. The min/max mirror the backend
 * clamp (`hybrid_rrf_k >= 1`; weights `[0, MAX_SEARCH_WEIGHT = 100]`;
 * `starred_boost` `[0, MAX_STARRED_BOOST = 0.5]`). The Rust `u32` RRF `k` has no
 * explicit upper clamp; the UI caps the slider at a sane 200 (well past the point
 * the fusion curve flattens) so the control stays usable — a hand-edited config
 * above that still loads, the UI just does not offer it.
 */
export const SEARCH_TUNING_BOUNDS: Record<
  SearchTuningKnob,
  { min: number; max: number; step: number }
> = {
  hybridRrfK: { min: 1, max: 200, step: 1 },
  lexicalWeight: { min: 0, max: 100, step: 0.1 },
  semanticWeight: { min: 0, max: 100, step: 0.1 },
  starredBoost: { min: 0, max: 0.5, step: 0.01 },
}

/**
 * Clamps one raw input value into the knob's valid range, resetting NaN to that
 * knob's default. `hybridRrfK` is additionally floored to an integer (the backend
 * field is a `u32`), so a fractional slider/keyboard value can never be persisted.
 *
 * Pure → unit-tested. Idempotent on an already-valid value.
 */
export function clampSearchTuningValue(
  knob: SearchTuningKnob,
  value: number,
): number {
  if (Number.isNaN(value)) {
    return SEARCH_TUNING_DEFAULTS[knob]
  }
  const { min, max } = SEARCH_TUNING_BOUNDS[knob]
  const clamped = Math.min(Math.max(value, min), max)
  return knob === 'hybridRrfK' ? Math.floor(clamped) : clamped
}

/**
 * Reads a knob off a draft, falling back to its default when the field is absent
 * (older snapshots ship the knobs as optional). Always returns a clamped value,
 * so the UI renders a legal control position even if a stored value drifted.
 */
export function resolveSearchTuningValue(
  settings: AiSettings | null | undefined,
  knob: SearchTuningKnob,
): number {
  const raw = settings?.[knob]
  if (raw === undefined || raw === null) {
    return SEARCH_TUNING_DEFAULTS[knob]
  }
  return clampSearchTuningValue(knob, raw)
}

/**
 * Returns a new draft with one knob set to the clamped/sanitized `value`. Other
 * knobs and every provider list are left referentially untouched (shallow spread).
 */
export function applySearchTuningKnob(
  settings: AiSettings,
  knob: SearchTuningKnob,
  value: number,
): AiSettings {
  return {
    ...settings,
    [knob]: clampSearchTuningValue(knob, value),
  }
}

/**
 * Returns a new draft with all four knobs restored to their accepted defaults.
 */
export function resetSearchTuningKnobs(settings: AiSettings): AiSettings {
  return {
    ...settings,
    hybridRrfK: SEARCH_TUNING_DEFAULTS.hybridRrfK,
    lexicalWeight: SEARCH_TUNING_DEFAULTS.lexicalWeight,
    semanticWeight: SEARCH_TUNING_DEFAULTS.semanticWeight,
    starredBoost: SEARCH_TUNING_DEFAULTS.starredBoost,
  }
}

/**
 * Whether the draft's knobs differ from their accepted defaults. Drives the
 * "Reset to defaults" affordance disabled state (no point resetting defaults).
 */
export function searchTuningDiffersFromDefaults(
  settings: AiSettings | null | undefined,
): boolean {
  return (
    resolveSearchTuningValue(settings, 'hybridRrfK') !==
      SEARCH_TUNING_DEFAULTS.hybridRrfK ||
    resolveSearchTuningValue(settings, 'lexicalWeight') !==
      SEARCH_TUNING_DEFAULTS.lexicalWeight ||
    resolveSearchTuningValue(settings, 'semanticWeight') !==
      SEARCH_TUNING_DEFAULTS.semanticWeight ||
    resolveSearchTuningValue(settings, 'starredBoost') !==
      SEARCH_TUNING_DEFAULTS.starredBoost
  )
}
