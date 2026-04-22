/**
 * @file catalog.ts
 * @description Re-exports the stable i18n catalog surface while implementation owners live under `src/lib/i18n/catalog/`.
 * @module lib/i18n
 *
 * ## Responsibilities
 * - Keep the public `src/lib/i18n/catalog.ts` import path stable for existing consumers.
 * - Re-export the public i18n types and runtime helpers from their split canonical owners.
 *
 * ## Not responsible for
 * - Owning translation strings directly
 * - Implementing translator or locale-resolution logic inline
 *
 * ## Dependencies
 * - `./catalog/catalog-types`
 * - `./catalog/catalog-runtime`
 *
 * ## Performance notes
 * - Thin barrel only; avoid adding runtime work here so repeated imports stay cheap.
 */

export * from './catalog/catalog-types'
export * from './catalog/catalog-runtime'
