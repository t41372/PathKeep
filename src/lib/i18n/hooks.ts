import { useContext } from 'react'
import type { TranslationNamespace } from './catalog'
import { I18nContext } from './context'

export function useI18nContext() {
  const value = useContext(I18nContext)
  if (!value) {
    throw new Error('useI18nContext must be used inside I18nProvider')
  }
  return value
}

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
