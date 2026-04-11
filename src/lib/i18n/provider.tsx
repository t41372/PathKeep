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
