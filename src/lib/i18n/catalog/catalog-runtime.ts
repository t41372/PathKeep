/**
 * @file catalog-runtime.ts
 * @description Assembles the split translation owners into the live catalog and exposes the stable translator helpers.
 * @module lib/i18n/catalog
 *
 * ## Responsibilities
 * - Assemble the split namespace owners into one canonical runtime catalog.
 * - Preserve translator fallback, interpolation, pseudo-locale, and locale-resolution behavior.
 * - Keep `translationCatalog()` available for tests and tooling without exposing the mutable singleton.
 *
 * ## Not responsible for
 * - Defining the public namespace/type unions
 * - Owning route-specific copy outside the imported namespace files
 *
 * ## Dependencies
 * - `../types` for `LanguagePreference`
 * - `catalog-types.ts` for the public i18n contract
 * - Split namespace owner files under `src/lib/i18n/catalog/`
 *
 * ## Performance notes
 * - Catalog assembly happens once at module load and then reuses a flattened in-memory lookup table.
 * - Keep helper logic allocation-light because this module backs every translated route render.
 */

import type { LanguagePreference } from '../../types'
import { assistantNamespaceCatalog } from './assistant'
import { auditNamespaceCatalog } from './audit'
import { commonNamespaceCatalog } from './common'
import { dashboardNamespaceCatalog } from './dashboard'
import { explorerNamespaceCatalog } from './explorer'
import { importNamespaceCatalog } from './import'
import { insightsNamespaceCatalog } from './insights'
import { intelligenceOverviewAndRoutesNamespace } from './intelligence-overview-and-routes'
import { intelligenceSearchAndRhythmNamespace } from './intelligence-search-and-rhythm'
import { intelligenceSecondaryMetaNamespace } from './intelligence-secondary-meta'
import { intelligenceSecondaryPatternsNamespace } from './intelligence-secondary-patterns'
import { intelligenceSessionTrailAndExplainNamespace } from './intelligence-session-trail-and-explain'
import { jobsNamespaceCatalog } from './jobs'
import { navigationNamespaceCatalog } from './navigation'
import { onboardingNamespaceCatalog } from './onboarding'
import { platformNamespaceCatalog } from './platform'
import { scheduleNamespaceCatalog } from './schedule'
import { securityNamespaceCatalog } from './security'
import { settingsAiProvidersNamespace } from './settings-ai-providers'
import { settingsAnalyticsAndUpdatesNamespace } from './settings-analytics-and-updates'
import { settingsCoreAndPlatformNamespace } from './settings-core-and-platform'
import { settingsDerivedAndRuntimeNamespace } from './settings-derived-and-runtime'
import { settingsRemoteAndOutputsNamespace } from './settings-remote-and-outputs'
import { shellNamespaceCatalog } from './shell'
import {
  supportedLanguages,
  type ResolvedLanguage,
  type TranslationKey,
  type TranslationNamespace,
} from './catalog-types'

type TranslationDictionary = {
  [key: string]: string | TranslationDictionary
}

type TranslationCatalog = Record<
  ResolvedLanguage,
  Record<TranslationNamespace, TranslationDictionary>
>

const catalog = {
  en: {
    common: commonNamespaceCatalog.en,
    shell: shellNamespaceCatalog.en,
    navigation: navigationNamespaceCatalog.en,
    dashboard: dashboardNamespaceCatalog.en,
    audit: auditNamespaceCatalog.en,
    import: importNamespaceCatalog.en,
    schedule: scheduleNamespaceCatalog.en,
    security: securityNamespaceCatalog.en,
    settings: {
      ...settingsCoreAndPlatformNamespace.en,
      ...settingsAnalyticsAndUpdatesNamespace.en,
      ...settingsRemoteAndOutputsNamespace.en,
      ...settingsDerivedAndRuntimeNamespace.en,
      ...settingsAiProvidersNamespace.en,
    },
    explorer: explorerNamespaceCatalog.en,
    assistant: assistantNamespaceCatalog.en,
    insights: insightsNamespaceCatalog.en,
    intelligence: {
      ...intelligenceOverviewAndRoutesNamespace.en,
      ...intelligenceSearchAndRhythmNamespace.en,
      ...intelligenceSessionTrailAndExplainNamespace.en,
      ...intelligenceSecondaryPatternsNamespace.en,
      ...intelligenceSecondaryMetaNamespace.en,
    },
    jobs: jobsNamespaceCatalog.en,
    platform: platformNamespaceCatalog.en,
    onboarding: onboardingNamespaceCatalog.en,
  },
  'zh-CN': {
    common: commonNamespaceCatalog['zh-CN'],
    shell: shellNamespaceCatalog['zh-CN'],
    navigation: navigationNamespaceCatalog['zh-CN'],
    dashboard: dashboardNamespaceCatalog['zh-CN'],
    audit: auditNamespaceCatalog['zh-CN'],
    import: importNamespaceCatalog['zh-CN'],
    schedule: scheduleNamespaceCatalog['zh-CN'],
    security: securityNamespaceCatalog['zh-CN'],
    settings: {
      ...settingsCoreAndPlatformNamespace['zh-CN'],
      ...settingsAnalyticsAndUpdatesNamespace['zh-CN'],
      ...settingsRemoteAndOutputsNamespace['zh-CN'],
      ...settingsDerivedAndRuntimeNamespace['zh-CN'],
      ...settingsAiProvidersNamespace['zh-CN'],
    },
    explorer: explorerNamespaceCatalog['zh-CN'],
    assistant: assistantNamespaceCatalog['zh-CN'],
    insights: insightsNamespaceCatalog['zh-CN'],
    intelligence: {
      ...intelligenceOverviewAndRoutesNamespace['zh-CN'],
      ...intelligenceSearchAndRhythmNamespace['zh-CN'],
      ...intelligenceSessionTrailAndExplainNamespace['zh-CN'],
      ...intelligenceSecondaryPatternsNamespace['zh-CN'],
      ...intelligenceSecondaryMetaNamespace['zh-CN'],
    },
    jobs: jobsNamespaceCatalog['zh-CN'],
    platform: platformNamespaceCatalog['zh-CN'],
    onboarding: onboardingNamespaceCatalog['zh-CN'],
  },
  'zh-TW': {
    common: commonNamespaceCatalog['zh-TW'],
    shell: shellNamespaceCatalog['zh-TW'],
    navigation: navigationNamespaceCatalog['zh-TW'],
    dashboard: dashboardNamespaceCatalog['zh-TW'],
    audit: auditNamespaceCatalog['zh-TW'],
    import: importNamespaceCatalog['zh-TW'],
    schedule: scheduleNamespaceCatalog['zh-TW'],
    security: securityNamespaceCatalog['zh-TW'],
    settings: {
      ...settingsCoreAndPlatformNamespace['zh-TW'],
      ...settingsAnalyticsAndUpdatesNamespace['zh-TW'],
      ...settingsRemoteAndOutputsNamespace['zh-TW'],
      ...settingsDerivedAndRuntimeNamespace['zh-TW'],
      ...settingsAiProvidersNamespace['zh-TW'],
    },
    explorer: explorerNamespaceCatalog['zh-TW'],
    assistant: assistantNamespaceCatalog['zh-TW'],
    insights: insightsNamespaceCatalog['zh-TW'],
    intelligence: {
      ...intelligenceOverviewAndRoutesNamespace['zh-TW'],
      ...intelligenceSearchAndRhythmNamespace['zh-TW'],
      ...intelligenceSessionTrailAndExplainNamespace['zh-TW'],
      ...intelligenceSecondaryPatternsNamespace['zh-TW'],
      ...intelligenceSecondaryMetaNamespace['zh-TW'],
    },
    jobs: jobsNamespaceCatalog['zh-TW'],
    platform: platformNamespaceCatalog['zh-TW'],
    onboarding: onboardingNamespaceCatalog['zh-TW'],
  },
} as const satisfies TranslationCatalog

function flattenDictionary(
  dictionary: TranslationDictionary,
  prefix = '',
  target: Record<string, string> = {},
) {
  for (const [key, value] of Object.entries(dictionary)) {
    const nextKey = prefix ? `${prefix}.${key}` : key
    if (typeof value === 'string') {
      target[nextKey] = value
      target[key] ??= value
      continue
    }

    flattenDictionary(value, nextKey, target)
  }

  return target
}

const flattenedCatalog = Object.fromEntries(
  supportedLanguages.map((language) => [
    language,
    flattenDictionary(catalog[language]),
  ]),
) as Record<ResolvedLanguage, Record<string, string>>

function interpolate(
  value: string,
  vars?: Record<string, string | number | null | undefined>,
) {
  if (!vars) {
    return value
  }

  return value.replaceAll(/\{([^}]+)\}/g, (match, key: string) => {
    const next = vars[key]
    return next === null || next === undefined ? match : String(next)
  })
}

/**
 * Returns a defensive clone of the full translation catalog so tests and tooling can inspect it safely.
 */
export function translationCatalog() {
  return structuredClone(catalog)
}

/**
 * Expands a translation string for pseudo-locale smoke tests while preserving runtime placeholder tokens.
 */
export function pseudoLocalize(value: string) {
  const accentMap: Record<string, string> = {
    a: 'á',
    e: 'ë',
    i: 'ï',
    o: 'ô',
    u: 'ü',
    A: 'Á',
    E: 'Ë',
    I: 'Ï',
    O: 'Ô',
    U: 'Ü',
  }

  const placeholders: string[] = []
  const withPlaceholdersPreserved = value.replaceAll(/\{[^}]+\}/g, (match) => {
    const token = `@@__${placeholders.length}__@@`
    placeholders.push(match)
    return token
  })

  const expanded = withPlaceholdersPreserved
    .split('')
    .map((char) => accentMap[char] ?? char)
    .join('')

  const restored = expanded.replaceAll(/@@__(\d+)__@@/g, (_, index: string) => {
    return placeholders[Number(index)] ?? ''
  })

  return `［${restored}］`
}

/**
 * Resolves the best supported UI language from browser locale hints while keeping traditional Chinese variants together.
 */
export function detectSystemLanguage(
  languages?: readonly string[],
): ResolvedLanguage {
  const preferred =
    languages ??
    (typeof navigator !== 'undefined'
      ? navigator.languages?.length
        ? navigator.languages
        : [navigator.language]
      : [])

  for (const locale of preferred) {
    const normalized = locale.toLowerCase()
    if (normalized.startsWith('zh')) {
      if (
        normalized.includes('tw') ||
        normalized.includes('hk') ||
        normalized.includes('mo') ||
        normalized.includes('hant')
      ) {
        return 'zh-TW'
      }
      return 'zh-CN'
    }

    if (normalized.startsWith('en')) {
      return 'en'
    }
  }

  return 'en'
}

/**
 * Chooses the concrete UI language that the app should render after applying follow-system fallback rules.
 */
export function resolveLanguage(
  preference?: LanguagePreference,
  languages?: readonly string[],
): ResolvedLanguage {
  if (!preference || preference === 'system') {
    return detectSystemLanguage(languages)
  }

  return supportedLanguages.includes(preference as ResolvedLanguage)
    ? preference
    : 'en'
}

/**
 * Creates the root translator used by routes and helpers, with English fallback preserved for missing keys.
 */
export function createTranslator(language: ResolvedLanguage, pseudo = false) {
  const dictionary = flattenedCatalog[language] ?? flattenedCatalog.en
  const fallback = flattenedCatalog.en

  return (key: TranslationKey, vars?: Record<string, string | number>) => {
    const value = dictionary[key] ?? fallback[key] ?? key
    const translated = interpolate(value, vars)
    return pseudo ? pseudoLocalize(translated) : translated
  }
}

/**
 * Narrows the root translator to one namespace so consumers can avoid repeating namespace prefixes by hand.
 */
export function createNamespaceTranslator(
  language: ResolvedLanguage,
  namespace: TranslationNamespace,
  pseudo = false,
) {
  const translate = createTranslator(language, pseudo)
  return (key: string, vars?: Record<string, string | number>) =>
    translate(`${namespace}.${key}`, vars)
}

/**
 * Returns the user-facing label for a language preference in the current UI language.
 */
export function languageLabel(
  preference: LanguagePreference,
  uiLanguage: ResolvedLanguage,
) {
  const translate = createTranslator(uiLanguage)
  if (preference === 'system') {
    return translate('common.followSystem')
  }
  if (preference === 'zh-CN') {
    return translate('common.simplifiedChinese')
  }
  if (preference === 'zh-TW') {
    return translate('common.traditionalChinese')
  }
  return translate('common.english')
}

/**
 * Maps the resolved PathKeep language to the locale tag that DOM and Intl APIs should receive.
 */
export function localeTag(language: ResolvedLanguage) {
  if (language === 'zh-CN') return 'zh-CN'
  if (language === 'zh-TW') return 'zh-TW'
  return 'en-US'
}
