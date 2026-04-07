import { useMemo, useState, type ReactNode } from 'react'
import type { LanguagePreference } from '../types'
import {
  createNamespaceTranslator,
  createTranslator,
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

  const value = useMemo<I18nContextValue>(() => {
    const t = createTranslator(language)

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
      ns: (namespace) => createNamespaceTranslator(language, namespace),
    }
  }, [language, preference])

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>
}
