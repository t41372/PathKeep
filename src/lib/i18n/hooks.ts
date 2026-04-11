/**
 * This module is part of PathKeep's shipping i18n contract, not a best-effort localization afterthought.
 *
 * Why this file exists:
 * - Every user-visible string, route label, callout, and loading surface should flow through this layer.
 * - Keeping the contract centralized makes it easier to reason about locale length, pseudo-locale smoke, and shared wording changes.
 *
 * Main declarations:
 * - `useI18nContext`
 * - `useI18n`
 *
 * Source-of-truth notes:
 * - Stay aligned with the i18n requirements in `docs/design/ux-principles.md`.
 * - The catalog must keep `en`, `zh-CN`, and `zh-TW` in sync for all shipped namespaces.
 */

import { useContext } from 'react'
import type { TranslationNamespace } from './catalog'
import { I18nContext } from './context'

/**
 * Provides the `useI18nContext` hook.
 *
 * This declaration is part of the shipping i18n contract, so clarity matters as much as correctness when new copy or namespaces are added.
 */
export function useI18nContext() {
  const value = useContext(I18nContext)
  if (!value) {
    throw new Error('useI18nContext must be used inside I18nProvider')
  }
  return value
}

/**
 * Provides the `useI18n` hook.
 *
 * This declaration is part of the shipping i18n contract, so clarity matters as much as correctness when new copy or namespaces are added.
 */
export function useI18n(namespace?: TranslationNamespace) {
  const value = useI18nContext()

  if (!namespace) {
    return {
      language: value.language,
      preference: value.preference,
      setLanguagePreference: value.setLanguagePreference,
      t: value.t,
      ns: value.ns,
    }
  }

  return {
    language: value.language,
    preference: value.preference,
    setLanguagePreference: value.setLanguagePreference,
    t: value.ns(namespace),
    ns: value.ns,
  }
}
