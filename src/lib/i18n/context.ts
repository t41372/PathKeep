import { createContext, useContext } from 'react'
import type { LanguagePreference } from '@/lib/types'
import type { ResolvedLanguage } from './define'
import type { Translator } from './runtime'

export interface I18nValue {
  language: ResolvedLanguage
  /** BCP 47 tag for Intl formatters. */
  locale: string
  preference: LanguagePreference
  t: Translator
  setPreference: (preference: LanguagePreference) => void
}

export const I18nContext = createContext<I18nValue | null>(null)

export function useI18n() {
  const value = useContext(I18nContext)
  if (!value) throw new Error('useI18n must be used inside I18nProvider')
  return value
}
