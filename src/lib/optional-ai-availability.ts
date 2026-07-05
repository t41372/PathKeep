/**
 * @file optional-ai-availability.ts
 * @description Centralizes the multi-condition gate that decides whether optional AI
 *              surfaces (semantic / hybrid recall, assistant, MCP, etc.) are usable
 *              right now, and explains the specific reason when they are not.
 * @module lib/optional-ai-availability
 *
 * ## Responsibilities
 * - Combine release-flag, embedding-provider, and runtime AI-state signals into a
 *   single `{ available, reason }` value the UI can consume in one read.
 * - Expose the specific reason so disabled buttons and callouts can tell the user
 *   the exact thing to fix instead of a generic "deferred" line. Each surface
 *   owns its own reason → copy mapping, because the reachable reason set differs
 *   per surface (e.g. the explorer renders only user-fixable repair states).
 * - Keep the gate itself release-fact aware without leaking provider config or
 *   queue snapshots into every consumer.
 *
 * ## Not responsible for
 * - Owning the user-visible strings — locale catalogs remain the source of truth.
 * - Triggering provider probes or kicking embedding builds; this module is pure.
 * - Deciding which surface should react to which reason; consumers keep that.
 *
 * ## Dependencies
 * - No runtime dependencies. The gate is intentionally a small pure function so
 *   tests stay deterministic and adoption stays cheap.
 *
 * ## Performance notes
 * - Pure synchronous compute. Importing this module must not trigger IO or
 *   backend calls.
 */

/**
 * Names the specific reason optional AI is currently unavailable.
 *
 * The reason exists so the UI can give the user a concrete next step instead of
 * a single "coming later" line that hides which dependency is the actual block.
 */
export type OptionalAiUnavailableReason =
  | 'release-deferred'
  | 'ai-disabled'
  | 'no-embedding-provider'
  | 'embedding-provider-error'

/**
 * Carries both the boolean gate and the specific reason it is closed so disabled
 * buttons, tooltips, and callouts can share one source of truth.
 */
export interface OptionalAiAvailability {
  available: boolean
  reason: OptionalAiUnavailableReason | null
}

/**
 * The set of AI index states that should hard-block optional AI surfaces because
 * the embedding pipeline is not delivering trustworthy semantic recall right
 * now.
 */
const ERROR_STATES = new Set(['failed', 'blocked', 'degraded'])

/**
 * Combines the three independent signals that have to be true before optional
 * AI surfaces may render their primary affordances.
 *
 * Order matters: callers see the most fundamental missing dependency first so a
 * user who has not selected a provider is never told to "check the provider
 * status" when there is no provider to check.
 */
export function evaluateOptionalAiAvailability(input: {
  releaseEnabled: boolean
  aiEnabled?: boolean
  embeddingProviderId: string | null
  aiStatusState?: string | null
}): OptionalAiAvailability {
  if (!input.releaseEnabled) {
    return { available: false, reason: 'release-deferred' }
  }
  if (input.aiEnabled === false || input.aiStatusState === 'disabled') {
    return { available: false, reason: 'ai-disabled' }
  }
  if (!input.embeddingProviderId) {
    return { available: false, reason: 'no-embedding-provider' }
  }
  if (input.aiStatusState && ERROR_STATES.has(input.aiStatusState)) {
    return { available: false, reason: 'embedding-provider-error' }
  }
  return { available: true, reason: null }
}
