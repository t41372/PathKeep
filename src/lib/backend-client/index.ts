/**
 * This module wraps a focused slice of desktop commands behind a typed front-end client.
 *
 * Why this file exists:
 * - The `backend-client` layer keeps page components from having to know raw command names or transport details.
 * - Re-exports every per-domain client and its types, so callers import from one place.
 *
 * Source-of-truth notes:
 * - Transport boundaries are defined by `docs/architecture/desktop-command-surface.md`.
 * - This layer should stay typed, boring, and free of user-facing copy so routes can keep ownership of UX decisions.
 */

export * from './annotations'
export * from './app'
export * from './archive'
export * from './audit'
export * from './content-enrichment'
export * from './dashboard'
export * from './desktop'
export * from './explorer'
export * from './import'
export * from './intelligence'
export * from './migration'
export * from './schedule'
export * from './security'
export * from './shared'
export * from './stars'
export * from './support'
export * from './update'
