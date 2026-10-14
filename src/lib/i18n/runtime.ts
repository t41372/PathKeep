/**
 * Translator runtime: flattens the catalogs once, then resolves keys with
 * `{param}` interpolation and `_one` / `_other` plural forms.
 */
import type { LanguagePreference } from '@/lib/types'
import { messages, type MessageKey } from './messages'
import {
  supportedLanguages,
  type MessageTree,
  type ResolvedLanguage,
} from './define'

export type TranslationParams = Record<string, string | number>
export type Translator = (key: MessageKey, params?: TranslationParams) => string

function flatten(
  tree: MessageTree,
  prefix = '',
  out = new Map<string, string>(),
) {
  for (const [key, value] of Object.entries(tree)) {
    const path = prefix ? `${prefix}.${key}` : key
    if (typeof value === 'string') out.set(path, value)
    else flatten(value, path, out)
  }
  return out
}

const flat = Object.fromEntries(
  supportedLanguages.map((lang) => [lang, flatten(messages[lang])]),
) as Record<ResolvedLanguage, Map<string, string>>

const pluralRules = Object.fromEntries(
  supportedLanguages.map((lang) => [lang, new Intl.PluralRules(lang)]),
) as Record<ResolvedLanguage, Intl.PluralRules>

export function createTranslator(lang: ResolvedLanguage): Translator {
  const table = flat[lang]
  const fallback = flat.en
  return (key, params) => {
    let template: string | undefined
    if (params && typeof params.count === 'number') {
      const form = pluralRules[lang].select(params.count)
      template =
        table.get(`${key}_${form}`) ??
        table.get(`${key}_other`) ??
        fallback.get(`${key}_${form === 'one' ? 'one' : 'other'}`)
    }
    template ??= table.get(key) ?? fallback.get(key)
    if (template === undefined) {
      if (import.meta.env.DEV) console.warn(`[i18n] missing key: ${key}`)
      return key
    }
    if (!params) return template
    return template.replace(/\{(\w+)\}/g, (match, name: string) => {
      const value = params[name]
      if (value === undefined) return match
      return typeof value === 'number'
        ? value.toLocaleString(localeTag(lang))
        : value
    })
  }
}

export function detectSystemLanguage(): ResolvedLanguage {
  const candidates =
    typeof navigator === 'undefined'
      ? []
      : [...(navigator.languages ?? []), navigator.language]
  for (const raw of candidates) {
    const tag = raw?.toLowerCase() ?? ''
    if (!tag.startsWith('zh')) {
      if (tag.startsWith('en')) return 'en'
      continue
    }
    if (/hant|tw|hk|mo/.test(tag)) return 'zh-TW'
    return 'zh-CN'
  }
  return 'en'
}

export function resolveLanguage(
  preference: LanguagePreference | null | undefined,
): ResolvedLanguage {
  if (preference && preference !== 'system') return preference
  return detectSystemLanguage()
}

export function localeTag(lang: ResolvedLanguage) {
  return lang === 'en' ? 'en-US' : lang
}

/** Each language's own name, shown in the language picker. */
export const languageNames: Record<ResolvedLanguage, string> = {
  en: 'English',
  'zh-CN': '简体中文',
  'zh-TW': '繁體中文',
}
