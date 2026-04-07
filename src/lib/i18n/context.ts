import { createContext } from 'react'
import type { LanguagePreference } from '../types'
import type {
  createNamespaceTranslator,
  createTranslator,
  ResolvedLanguage,
  TranslationNamespace,
} from './catalog'

export const i18nStorageKey = 'pathkeep-language-preference'

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

export const I18nContext = createContext<I18nContextValue | null>(null)

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
