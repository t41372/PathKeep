/**
 * This module re-exports the typed front-end contracts that the rest of the shell imports as a single surface.
 *
 * Why this file exists:
 * - The UI reads these shapes as its desktop and preview contract, so unclear names here ripple through every consumer.
 * - If you need to know what a route or helper expects from the backend, this is often the fastest file to open first.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Data shapes should stay aligned with the accepted architecture and feature docs rather than ad-hoc page assumptions.
 * - Prefer additive, explicit fields over ambiguous catch-all objects so the trust surface stays auditable.
 */

export * from './app'
export * from './archive'
export * from './audit'
export * from './import'
export * from './intelligence'
export * from './remote'
export * from './schedule'
export * from './security'
