/**
 * @file release-capabilities.ts
 * @description Centralizes release-scope capability flags for surfaces moved to the v0.3 roadmap.
 * @module lib/release-capabilities
 *
 * ## Responsibilities
 * - Keep UI guards for optional AI and readable-content fetch features tied to one release fact.
 * - Make v0.2.0 disabled states and v0.3 roadmap labels explicit without deleting future-facing route shells.
 * - Provide a non-localized release label for code paths that only need a stable version marker.
 *
 * ## Not responsible for
 * - Owning user-visible copy; locale catalogs remain the source of truth for text.
 * - Deciding backend provider or vector-store behavior.
 * - Enabling hidden runtime work while UI controls are disabled.
 *
 * ## Dependencies
 * - No runtime dependencies. This module must stay small and side-effect free.
 *
 * ## Performance notes
 * - Static booleans only; importing this file must never trigger IO or backend calls.
 */

/**
 * Marks the optional AI surfaces as available to *configure* in this release.
 *
 * `true` only lifts the v0.2 blackout that hid the AI configuration front door;
 * it does NOT turn AI on. Every actual AI operation (provider probes, embedding
 * builds, semantic search, assistant chat, MCP, skill previews) stays gated by
 * the user's own off-by-default `config.ai.enabled` consent. The flag answers
 * "may the user see and edit the AI settings?", never "is AI running?".
 */
export const optionalAiFeaturesAvailable = true

/**
 * Marks network-backed readable-content fetching as tracked for v0.3 instead of v0.2.0.
 *
 * Core archive, keyword search, and deterministic Core Intelligence must remain
 * usable without claiming that PathKeep can fetch and store webpage bodies.
 */
export const readableContentFetchAvailable = false

/**
 * Names the next planned release bucket for disabled future-facing surfaces.
 */
export const deferredFeatureReleaseLabel = 'v0.3'
