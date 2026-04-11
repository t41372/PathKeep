/**
 * This module is part of PathKeep's shipping i18n contract, not a best-effort localization afterthought.
 *
 * Why this file exists:
 * - Every user-visible string, route label, callout, and loading surface should flow through this layer.
 * - Keeping the contract centralized makes it easier to reason about locale length, pseudo-locale smoke, and shared wording changes.
 *
 * Main declarations:
 * - `i18nStorageKey`
 * - `I18nContextValue`
 * - `I18nContext`
 * - `readStoredPreference`
 *
 * Source-of-truth notes:
 * - Stay aligned with the i18n requirements in `docs/design/ux-principles.md`.
 * - The catalog must keep `en`, `zh-CN`, and `zh-TW` in sync for all shipped namespaces.
 */

import { createContext } from 'react'
import type { LanguagePreference } from '../types'
import type {
  createNamespaceTranslator,
  createTranslator,
  ResolvedLanguage,
  TranslationNamespace,
} from './catalog'

export const i18nStorageKey = 'pathkeep-language-preference'

/**
 * Defines the value exposed through the `I18nContext` context.
 *
 * This declaration is part of the shipping i18n contract, so clarity matters as much as correctness when new copy or namespaces are added.
 */
export interface I18nContextValue {
  language: ResolvedLanguage
  preference: LanguagePreference
  setLanguagePreference: (
    preference: LanguagePreference,
    options?: { persist?: boolean },
  ) => void
  t: ReturnType<typeof createTranslator>
  ns: (
    namespace: TranslationNamespace,
  ) => ReturnType<typeof createNamespaceTranslator>
}

/**
 * Holds the React context used to share i18n across the shell.
 *
 * This declaration is part of the shipping i18n contract, so clarity matters as much as correctness when new copy or namespaces are added.
 */
export const I18nContext = createContext<I18nContextValue | null>(null)

/**
 * Reads stored preference from the current runtime.
 *
 * This declaration is part of the shipping i18n contract, so clarity matters as much as correctness when new copy or namespaces are added.
 */
export function readStoredPreference(): LanguagePreference {
  if (typeof window === 'undefined') {
    return 'system'
  }

  const stored = window.localStorage.getItem(i18nStorageKey)
  if (
    stored === 'system' ||
    stored === 'en' ||
    stored === 'zh-CN' ||
    stored === 'zh-TW'
  ) {
    return stored
  }

  return 'system'
}
