/**
 * Language state for the whole app. The preference lives in localStorage so
 * the first paint is already in the right language; the shell mirrors it into
 * the backend config (`preferredLanguage`) for the scheduler and worker.
 */
import { useEffect, useMemo, useState, type ReactNode } from 'react'
import type { LanguagePreference } from '@/lib/types'
import { I18nContext } from './context'
import { createTranslator, localeTag, resolveLanguage } from './runtime'

const STORAGE_KEY = 'pathkeep-language-preference'
const preferences: LanguagePreference[] = ['system', 'en', 'zh-CN', 'zh-TW']

export function readStoredLanguagePreference(): LanguagePreference {
  try {
    const value = localStorage.getItem(STORAGE_KEY)
    if (preferences.includes(value as LanguagePreference)) {
      return value as LanguagePreference
    }
  } catch {
    // Storage can be unavailable in hardened WebViews.
  }
  return 'system'
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const [preference, setPreference] = useState(readStoredLanguagePreference)
  const language = resolveLanguage(preference)

  useEffect(() => {
    document.documentElement.lang = localeTag(language)
  }, [language])

  const value = useMemo(
    () => ({
      language,
      locale: localeTag(language),
      preference,
      t: createTranslator(language),
      setPreference: (next: LanguagePreference) => {
        try {
          localStorage.setItem(STORAGE_KEY, next)
        } catch {
          // Keep the in-memory choice even if it cannot be persisted.
        }
        setPreference(next)
      },
    }),
    [language, preference],
  )

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>
}
