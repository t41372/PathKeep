/**
 * @file catalog-types.ts
 * @description Defines the public type and namespace surface for PathKeep's shipping translation catalog.
 * @module lib/i18n/catalog
 *
 * ## Responsibilities
 * - Publish the supported language union that every i18n consumer shares.
 * - Publish the stable namespace list used by provider caches, tests, and route helpers.
 * - Keep the public i18n contract small and stable while implementation owners move underneath it.
 *
 * ## Not responsible for
 * - Storing translation strings
 * - Building translators, flattening dictionaries, or resolving locale preferences
 *
 * ## Dependencies
 * - No runtime dependencies; `catalog-runtime.ts` and external i18n consumers import these declarations.
 *
 * ## Performance notes
 * - Static constants only; keep this file side-effect free so i18n type imports stay cheap.
 */

/**
 * Names the concrete UI languages PathKeep can render after resolving any system-following preference.
 */
export type ResolvedLanguage = 'en' | 'zh-CN' | 'zh-TW'

/**
 * Lists the top-level translation namespaces that ship as part of PathKeep's UI contract.
 */
export type TranslationNamespace =
  | 'common'
  | 'shell'
  | 'navigation'
  | 'dashboard'
  | 'audit'
  | 'import'
  | 'schedule'
  | 'security'
  | 'settings'
  | 'explorer'
  | 'assistant'
  | 'insights'
  | 'intelligence'
  | 'jobs'
  | 'platform'
  | 'onboarding'

/**
 * Leaves room for legacy flat-key callers while still documenting that translator keys are string paths.
 */
export type TranslationKey = string

/**
 * Preserves the only languages the shipped front-end should ever resolve to at runtime.
 */
export const supportedLanguages: ResolvedLanguage[] = ['en', 'zh-CN', 'zh-TW']

/**
 * Keeps namespace iteration deterministic for provider caches, tests, and alignment checks.
 */
export const translationNamespaces: TranslationNamespace[] = [
  'common',
  'shell',
  'navigation',
  'dashboard',
  'audit',
  'import',
  'schedule',
  'security',
  'settings',
  'explorer',
  'assistant',
  'insights',
  'intelligence',
  'jobs',
  'platform',
  'onboarding',
]
