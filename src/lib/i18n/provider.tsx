/**
 * This module is part of PathKeep's shipping i18n contract, not a best-effort localization afterthought.
 *
 * Why this file exists:
 * - Every user-visible string, route label, callout, and loading surface should flow through this layer.
 * - Keeping the contract centralized makes it easier to reason about locale length, pseudo-locale smoke, and shared wording changes.
 *
 * Main declarations:
 * - `I18nProvider`
 *
 * Source-of-truth notes:
 * - Stay aligned with the i18n requirements in `docs/design/ux-principles.md`.
 * - The catalog must keep `en`, `zh-CN`, and `zh-TW` in sync for all shipped namespaces.
 */

import { useEffect, useMemo, useState, type ReactNode } from 'react'
import type { LanguagePreference } from '../types'
import {
  createNamespaceTranslator,
  createTranslator,
  localeTag,
  resolveLanguage,
} from './catalog'
import {
  I18nContext,
  i18nStorageKey,
  readStoredPreference,
  type I18nContextValue,
} from './context'

/**
 * Provides i18n to descendant components.
 *
 * This declaration is part of the shipping i18n contract, so clarity matters as much as correctness when new copy or namespaces are added.
 */
export function I18nProvider({ children }: { children: ReactNode }) {
  const [preference, setPreference] = useState<LanguagePreference>(() =>
    readStoredPreference(),
  )
  const language = resolveLanguage(preference)

  useEffect(() => {
    if (typeof document === 'undefined') {
      return
    }

    document.documentElement.lang = localeTag(language)
  }, [language])

  const value = useMemo<I18nContextValue>(() => {
    const t = createTranslator(language)
    const namespaceCache = new Map<
      Parameters<I18nContextValue['ns']>[0],
      ReturnType<I18nContextValue['ns']>
    >()
    /**
     * Explains how ns works.
     *
     * This declaration is part of the shipping i18n contract, so clarity matters as much as correctness when new copy or namespaces are added.
     */
    const ns: I18nContextValue['ns'] = (namespace) => {
      const cached = namespaceCache.get(namespace)
      if (cached) {
        return cached
      }

      const translator = createNamespaceTranslator(language, namespace)
      namespaceCache.set(namespace, translator)
      return translator
    }

    return {
      language,
      preference,
      setLanguagePreference: (nextPreference, options) => {
        setPreference(nextPreference)
        if (options?.persist === false || typeof window === 'undefined') {
          return
        }
        window.localStorage.setItem(i18nStorageKey, nextPreference)
      },
      t,
      ns,
    }
  }, [language, preference])

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>
}
