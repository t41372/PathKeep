/**
 * @file i18n-progress.ts
 * @description Reports shipped i18n key parity and blocks known raw-English leakage in Chinese locales.
 *
 * ## Responsibilities
 * - Count every shipped translation key by locale and namespace.
 * - Fail the check when `zh-CN` or `zh-TW` is missing keys present in English.
 * - Fail the check when Chinese locales contain known raw backend/debug phrases that must be localized before display.
 *
 * ## Not responsible for
 * - Judging acceptable product names such as PathKeep, Safari, Chrome, macOS, SQLCipher, or URLs.
 * - Replacing route/component tests that verify a specific string is actually rendered.
 * - Performing machine translation.
 *
 * ## Dependencies
 * - Imports the canonical runtime catalog from `src/lib/i18n/catalog`.
 *
 * ## Performance notes
 * - Static catalog walk only. This is intentionally cheap enough to run in the JS check gate.
 */

import {
  supportedLanguages,
  translationCatalog,
  translationNamespaces,
  type ResolvedLanguage,
} from '../src/lib/i18n/catalog'

type TranslationNode = string | Record<string, TranslationNode>

interface FlatEntry {
  key: string
  value: string
}

interface MissingKeyIssue {
  language: ResolvedLanguage
  key: string
}

interface RawEnglishIssue {
  language: Exclude<ResolvedLanguage, 'en'>
  key: string
  patternId: string
  value: string
}

const chineseLocales = ['zh-CN', 'zh-TW'] as const

const blockedRawEnglishPatterns = [
  {
    id: 'full-disk-access-english',
    pattern: /\bFull Disk Access\b/i,
  },
  {
    id: 'safari-access-raw-error',
    pattern: /Safari History\.db is not readable yet/i,
  },
  {
    id: 'grant-full-disk-access',
    pattern: /Grant Full Disk Access/i,
  },
  {
    id: 'archive-facts-debug-label',
    pattern: /\barchive facts\b/i,
  },
  {
    id: 'canonical-archive-run-debug-label',
    pattern: /\bcanonical archive run\b/i,
  },
  {
    id: 'shell-state-debug-label',
    pattern: /\bshell state\b/i,
  },
  {
    id: 'copying-source-debug-log',
    pattern: /\bcopying\s+\/Users\//i,
  },
  {
    id: 'visible-profile-jargon',
    pattern: /\bprofile\b/i,
  },
  {
    id: 'visible-adapter-jargon',
    pattern: /\badapter\b/i,
  },
  {
    id: 'visible-append-only-jargon',
    pattern: /\bappend-only\b/i,
  },
  {
    id: 'missing-key-leak',
    pattern: /^[a-z]+(?:\.[a-z][a-z0-9]*){1,}$/i,
  },
  {
    id: 'app-data-jargon',
    pattern: /\bapp data\b/i,
  },
] as const

function removeInterpolationPlaceholders(value: string) {
  return value.replace(/\{[a-zA-Z0-9_]+\}/g, '')
}

function flattenCatalogNode(
  node: TranslationNode,
  prefix = '',
  entries: FlatEntry[] = [],
) {
  if (typeof node === 'string') {
    entries.push({ key: prefix, value: node })
    return entries
  }

  const dictionary = node as Record<string, TranslationNode>
  for (const [key, value] of Object.entries(dictionary)) {
    flattenCatalogNode(value, prefix ? `${prefix}.${key}` : key, entries)
  }

  return entries
}

function formatPercent(value: number) {
  return `${value.toFixed(2)}%`
}

const catalog = translationCatalog()
const flatCatalog = Object.fromEntries(
  supportedLanguages.map((language) => [
    language,
    flattenCatalogNode(catalog[language] as TranslationNode),
  ]),
) as Record<ResolvedLanguage, FlatEntry[]>

const keySets = Object.fromEntries(
  supportedLanguages.map((language) => [
    language,
    new Set(flatCatalog[language].map((entry) => entry.key)),
  ]),
) as Record<ResolvedLanguage, Set<string>>

const englishKeys = [...keySets.en].sort()
const missingKeyIssues: MissingKeyIssue[] = []

for (const language of supportedLanguages) {
  for (const key of englishKeys) {
    if (!keySets[language].has(key)) {
      missingKeyIssues.push({ language, key })
    }
  }
}

const rawEnglishIssues: RawEnglishIssue[] = []

for (const language of chineseLocales) {
  for (const entry of flatCatalog[language]) {
    const visibleValue = removeInterpolationPlaceholders(entry.value)
    for (const blocked of blockedRawEnglishPatterns) {
      if (blocked.pattern.test(visibleValue)) {
        rawEnglishIssues.push({
          language,
          key: entry.key,
          patternId: blocked.id,
          value: entry.value,
        })
      }
    }
  }
}

const totalEnglishKeys = englishKeys.length
const completeLocaleCount = supportedLanguages.filter(
  (language) => keySets[language].size === totalEnglishKeys,
).length
const parityPercent =
  supportedLanguages.length === 0
    ? 100
    : (completeLocaleCount / supportedLanguages.length) * 100

console.log(
  `i18n namespaces: ${translationNamespaces.length} (${translationNamespaces.join(', ')})`,
)
console.log(
  `i18n key parity: ${formatPercent(parityPercent)} (${completeLocaleCount}/${supportedLanguages.length} locales complete, ${totalEnglishKeys} English keys)`,
)
for (const language of supportedLanguages) {
  console.log(`i18n ${language}: ${keySets[language].size} keys`)
}
console.log(`i18n missing keys: ${missingKeyIssues.length}`)
console.log(`i18n blocked raw-English findings: ${rawEnglishIssues.length}`)

for (const issue of missingKeyIssues.slice(0, 20)) {
  console.error(`[missing:${issue.language}] ${issue.key}`)
}
for (const issue of rawEnglishIssues.slice(0, 20)) {
  console.error(
    `[raw-English:${issue.language}:${issue.patternId}] ${issue.key} = ${issue.value}`,
  )
}

if (missingKeyIssues.length > 0 || rawEnglishIssues.length > 0) {
  process.exitCode = 1
}
