/**
 * @file heuristics.ts
 * @description Shared filtering and normalization rules for secondary intelligence overview sections.
 * @module pages/intelligence/sections/secondary-sections
 *
 * ## Responsibilities
 * - Keep low-signal secondary cards honest by filtering noisy or empty data.
 * - Normalize repeated intelligence heuristics so section modules do not drift.
 * - Provide small formatting helpers that are part of section-specific interpretation rules.
 *
 * ## Non-Responsibilities
 * - Does not fetch intelligence data or manage React state.
 * - Does not render UI or own route composition.
 * - Does not define cross-route navigation grammar.
 *
 * ## Dependencies
 * - `lib/core-intelligence` for typed deterministic overview entities.
 * - `../shared` for the route-local translation contract used by section labels.
 *
 * ## Performance Notes
 * - These helpers run on already-bounded overview payloads, not unbounded history streams.
 * - Filters stay allocation-light because they may run during staged overview renders.
 */

import type {
  FrictionSignal,
  PathFlow,
  ReopenedInvestigation,
  StableSource,
} from '../../../../lib/core-intelligence'
import type { T } from '../shared'

/**
 * Keeps stable-source cards hidden unless both entry and landing evidence exist.
 *
 * This prevents the secondary grid from spending space on half-formed signals
 * that read like raw metrics instead of an actual pattern.
 *
 * @param entries Stable entry domains derived from repeated trail starts.
 * @param landings Stable landing domains derived from repeated trail finishes.
 * @returns `true` when both sides of the pattern are present and worth showing.
 */
export function hasMeaningfulStableSources(
  entries: StableSource[],
  landings: StableSource[],
) {
  return entries.length > 0 && landings.length > 0
}

/**
 * Drops weak friction signals that would otherwise create noisy "something felt bad"
 * cards with too little evidence to act on.
 *
 * @param signal A candidate friction signal from deterministic analysis.
 * @returns `true` when the signal is strong enough to deserve user attention.
 */
export function isMeaningfulFrictionSignal(signal: FrictionSignal) {
  if (!signal.description.trim()) {
    return false
  }

  return (
    signal.evidenceType === 'strong' ||
    (signal.occurrenceCount >= 2 &&
      ['bounce_pattern', 'excessive_reformulation', 'redirect_chain'].includes(
        signal.signalKind,
      ))
  )
}

/**
 * Filters reopened investigations down to search-backed questions instead of
 * navigational or login noise.
 *
 * @param item One reopened investigation candidate from the overview payload.
 * @returns `true` when the item looks like a recurring question worth surfacing.
 */
export function isSearchBackedReopenedInvestigation(
  item: ReopenedInvestigation,
) {
  const label = item.anchorLabel.trim()
  if (item.anchorType !== 'query_family') {
    return false
  }
  if (item.occurrenceCount < 2 || item.distinctDays < 2) {
    return false
  }
  if (!label || looksLikeUrlOrDomain(label)) {
    return false
  }

  return /[\s?]/.test(label) || label.length >= 12
}

/**
 * Keeps path-flow cards focused on real browsing sequences instead of auth
 * redirects or same-site loops.
 *
 * @param flow One deterministic path-flow summary.
 * @returns `true` when the flow contains repeated, cross-domain browsing intent.
 */
export function isMeaningfulPathFlow(flow: PathFlow) {
  const steps = flow.flowPattern.split(/\s*(?:->|→)\s*/).filter(Boolean)
  if (flow.occurrenceCount < 2 || steps.length < 2) {
    return false
  }
  if (steps.some(isUtilityFlowStep)) {
    return false
  }

  const normalizedSteps = steps.map(normalizeFlowStep)
  if (new Set(normalizedSteps).size < 2) {
    return false
  }

  return normalizedSteps.every(
    (step, index) => index === 0 || step !== normalizedSteps[index - 1],
  )
}

/**
 * Turns ISO week keys into localized labels without forcing section modules to
 * know the route's translation grammar.
 *
 * @param dateKey Backend week key in `YYYY-Www` format.
 * @param t Route-local intelligence translator.
 * @returns A localized week label, or the raw key when the input is malformed.
 */
export function humanizeDiscoveryWeekLabel(dateKey: string, t: T) {
  const match = /^(\d{4})-W(\d{2})$/.exec(dateKey)
  if (!match) {
    return dateKey
  }

  return t('discoveryTrendWeekLabel', {
    year: Number(match[1]),
    week: Number(match[2]),
  })
}

function looksLikeUrlOrDomain(label: string) {
  const normalized = label.trim().toLowerCase()
  if (!normalized) {
    return false
  }

  return (
    normalized.includes('://') ||
    /^www\./.test(normalized) ||
    /\b[a-z0-9-]+(?:\.[a-z0-9-]+){1,}\b/.test(normalized) ||
    normalized.includes('/auth/') ||
    normalized.includes('/login') ||
    normalized.includes('callback')
  )
}

function normalizeFlowStep(step: string) {
  const normalized = step
    .trim()
    .toLowerCase()
    .replace(/^[a-z]+:\/\//, '')
    .replace(/\/.*$/, '')
    .replace(/^www\./, '')
    .replace(/^m\./, '')
    .replace(/^amp\./, '')

  if (
    normalized.includes('chat.openai.') ||
    normalized.includes('chatgpt.com')
  ) {
    return 'chatgpt'
  }
  if (normalized.includes('twitter.com') || normalized.includes('x.com')) {
    return 'x.com'
  }

  return normalized
}

function isUtilityFlowStep(step: string) {
  const normalized = step.trim().toLowerCase()
  return (
    normalized.includes('localhost') ||
    normalized.includes('callback') ||
    normalized.includes('oauth') ||
    normalized.includes('consent') ||
    normalized.includes('login') ||
    normalized.includes('sign-in') ||
    normalized.includes('signin') ||
    normalized.includes('auth.')
  )
}
