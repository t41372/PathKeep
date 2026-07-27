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
  language: ResolvedLanguage
  key: string
  patternId: string
  value: string
}

const chineseLocales = ['zh-CN', 'zh-TW'] as const

/**
 * Blocks raw backend/debug English that leaked into the Chinese catalogs.
 *
 * Every entry here was an actual shipped leak, not a hypothetical: the gate is
 * only useful if a regression is what turns it red, so keep the list tied to
 * vocabulary the product has already decided not to show a Chinese reader.
 */
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
  {
    id: 'rollup-jargon',
    pattern: /\brollups?\b/i,
  },
  {
    id: 'digest-jargon',
    pattern: /\bdigests?\b/i,
  },
  {
    id: 'worker-jargon',
    pattern: /\bworkers?\b/i,
  },
  {
    id: 'payload-jargon',
    pattern: /\bpayloads?\b/i,
  },
  {
    id: 'schema-jargon',
    pattern: /\bschemas?\b/i,
  },
  {
    id: 'migration-jargon',
    pattern: /\bmigrations?\b/i,
  },
  {
    id: 'archive-wide-jargon',
    pattern: /\barchive-wide\b/i,
  },
  {
    id: 'route-grammar-jargon',
    pattern: /\broute grammar\b/i,
  },
  {
    id: 'enrichment-jargon',
    pattern: /\benrichment\b/i,
  },
  {
    id: 'trace-jargon',
    pattern: /\btraces?\b/i,
  },
  {
    id: 'visit-jargon',
    pattern: /\bvisits?\b/i,
  },
  {
    id: 'archive-jargon',
    pattern: /\barchives?\b/i,
  },
  {
    id: 'checkpoint-jargon',
    pattern: /\bcheckpoints?\b/i,
  },
] as const

/**
 * Blocks implementation vocabulary on the English side of the catalog.
 *
 * Chinese leakage is easy to spot because the surrounding text is Chinese;
 * English jargon hides in plain sight, so the terms the product has already
 * renamed for users get a gate of their own rather than a style-guide note.
 */
const blockedEnglishJargonPatterns = [
  {
    id: 'en-shell-state-jargon',
    pattern: /\bshell state\b/i,
  },
  {
    id: 'en-append-only-jargon',
    pattern: /\bappend-only\b/i,
  },
  {
    id: 'en-app-data-jargon',
    pattern: /\bapp data\b/i,
  },
  {
    id: 'en-canonical-place-jargon',
    pattern: /\bcanonical place\b/i,
  },
  {
    id: 'en-profile-scope-jargon',
    pattern: /\bprofile scope\b/i,
  },
  {
    id: 'en-all-profiles-jargon',
    pattern: /\ball profiles\b/i,
  },
  {
    id: 'en-article-before-embedding',
    pattern: /\ba embedding\b/i,
  },
] as const

/**
 * Blocks mainland-Chinese vocabulary and stale OS names from the `zh-TW` catalog.
 *
 * `zh-TW` had already converged on the Taiwanese term for each of these in
 * other strings, so a single locale-scoped gate is what stops the mixed pairs
 * from drifting back in one string at a time.
 */
const blockedTraditionalChinesePatterns = [
  {
    id: 'zh-tw-stale-full-disk-access-name',
    pattern: /完全磁碟|磁碟存取權(?!限)/,
  },
  {
    id: 'zh-tw-mainland-vocabulary',
    pattern: /會話|視圖|訪問|信號|智能|卸載|刷新|決定性|語義/,
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

function collectIssues(
  language: ResolvedLanguage,
  patterns: ReadonlyArray<{ id: string; pattern: RegExp }>,
) {
  for (const entry of flatCatalog[language]) {
    const visibleValue = removeInterpolationPlaceholders(entry.value)
    for (const blocked of patterns) {
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

for (const language of chineseLocales) {
  collectIssues(language, blockedRawEnglishPatterns)
}
collectIssues('zh-TW', blockedTraditionalChinesePatterns)
collectIssues('en', blockedEnglishJargonPatterns)

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
