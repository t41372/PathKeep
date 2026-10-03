/**
 * i18n gate (`bun run check:i18n`).
 *
 * The compiler already guarantees the three catalogs have the same keys
 * (see `defineMessages`). This script checks what the compiler cannot:
 * - every translation uses the same {placeholders} as English
 * - no catalog still contains a TODO stub
 * - Chinese copy has no untranslated English jargon
 * - Traditional Chinese uses Taiwan vocabulary, not mainland terms
 * - English copy avoids internal jargon
 */
import {
  messages,
  supportedLanguages,
  type MessageTree,
  type ResolvedLanguage,
} from '../src/lib/i18n/messages-entry'

interface Rule {
  id: string
  pattern: RegExp
}

// Internal words that must not leak into Chinese UI copy untranslated.
// Product names (Chrome, Safari, Google Takeout, MCP, AI, API) are fine.
const chineseRawEnglish: Rule[] = [
  { id: 'full-disk-access', pattern: /\bFull Disk Access\b/i },
  { id: 'profile', pattern: /\bprofiles?\b/i },
  { id: 'adapter', pattern: /\badapters?\b/i },
  { id: 'append-only', pattern: /\bappend-only\b/i },
  { id: 'rollup', pattern: /\brollups?\b/i },
  { id: 'digest', pattern: /\bdigests?\b/i },
  { id: 'worker', pattern: /\bworkers?\b/i },
  { id: 'payload', pattern: /\bpayloads?\b/i },
  { id: 'schema', pattern: /\bschemas?\b/i },
  { id: 'migration', pattern: /\bmigrations?\b/i },
  { id: 'enrichment', pattern: /\benrichment\b/i },
  { id: 'trace', pattern: /\btraces?\b/i },
  { id: 'visit', pattern: /\bvisits?\b/i },
  { id: 'archive', pattern: /\barchives?\b/i },
  { id: 'checkpoint', pattern: /\bcheckpoints?\b/i },
]

const taiwanVocabulary: Rule[] = [
  { id: 'stale-full-disk-access-name', pattern: /完全磁碟|磁碟存取權(?!限)/ },
  {
    id: 'mainland-vocabulary',
    pattern: /會話|視圖|訪問|信號|智能|卸載|刷新|決定性|語義/,
  },
]

const englishJargon: Rule[] = [
  { id: 'shell-state', pattern: /\bshell state\b/i },
  { id: 'append-only', pattern: /\bappend-only\b/i },
  { id: 'app-data', pattern: /\bapp data\b/i },
  { id: 'canonical-place', pattern: /\bcanonical place\b/i },
  { id: 'profile-scope', pattern: /\bprofile scope\b/i },
  { id: 'all-profiles', pattern: /\ball profiles\b/i },
]

const everyLanguage: Rule[] = [
  { id: 'todo-stub', pattern: /^TODO$|\bTODO\b/ },
  { id: 'key-leak', pattern: /^[a-z]+(?:\.[a-z][a-zA-Z0-9]*)+$/ },
]

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

const placeholders = (value: string) =>
  [...value.matchAll(/\{(\w+)\}/g)]
    .map((match) => match[1])
    .sort()
    .join(',')
const visible = (value: string) => value.replace(/\{\w+\}/g, '')

const flat = Object.fromEntries(
  supportedLanguages.map((language) => [language, flatten(messages[language])]),
) as Record<ResolvedLanguage, Map<string, string>>

const problems: string[] = []

for (const language of supportedLanguages) {
  for (const key of flat.en.keys()) {
    if (!flat[language].has(key)) problems.push(`[missing:${language}] ${key}`)
  }
}

for (const language of ['zh-CN', 'zh-TW'] as const) {
  for (const [key, value] of flat[language]) {
    const english = flat.en.get(key)
    // Plural forms may legitimately drop {count} in one language, e.g. "one visit".
    const pluralForm = /_(one|other)$/.test(key)
    if (
      english !== undefined &&
      !pluralForm &&
      placeholders(english) !== placeholders(value)
    ) {
      problems.push(
        `[placeholders:${language}] ${key}: "${value}" vs en "${english}"`,
      )
    }
  }
}

function check(language: ResolvedLanguage, rules: Rule[]) {
  for (const [key, value] of flat[language]) {
    for (const rule of rules) {
      if (
        rule.pattern.test(visible(value)) ||
        (rule.id === 'key-leak' && rule.pattern.test(value))
      ) {
        problems.push(`[${rule.id}:${language}] ${key} = ${value}`)
      }
    }
  }
}

check('zh-CN', chineseRawEnglish)
check('zh-TW', chineseRawEnglish)
check('zh-TW', taiwanVocabulary)
check('en', englishJargon)
for (const language of supportedLanguages) check(language, everyLanguage)

console.log(
  `i18n: ${flat.en.size} keys × ${supportedLanguages.length} languages, ${problems.length} problem(s)`,
)
for (const problem of problems.slice(0, 50)) console.error(problem)
if (problems.length > 50) console.error(`…and ${problems.length - 50} more`)
if (problems.length > 0) process.exitCode = 1
