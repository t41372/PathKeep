import { describe, expect, test } from 'vitest'

import {
  createTranslator,
  detectSystemLanguage,
  languageLabel,
  localeTag,
  pseudoLocalize,
  resolveLanguage,
  translationCatalog,
  translationNamespaces,
} from './i18n'

function collectLeafKeys(
  node: Record<string, string | Record<string, unknown>>,
  prefix = '',
  keys: string[] = [],
) {
  for (const [key, value] of Object.entries(node)) {
    const next = prefix ? `${prefix}.${key}` : key
    if (typeof value === 'string') {
      keys.push(next)
      continue
    }
    collectLeafKeys(
      value as Record<string, string | Record<string, unknown>>,
      next,
      keys,
    )
  }

  return keys.sort()
}

describe('i18n helpers', () => {
  test('detects traditional and simplified Chinese variants correctly', () => {
    expect(detectSystemLanguage(['zh-Hant-TW'])).toBe('zh-TW')
    expect(detectSystemLanguage(['zh-CN'])).toBe('zh-CN')
    expect(detectSystemLanguage(['en-US'])).toBe('en')
  })

  test('resolves explicit and system language preferences', () => {
    expect(resolveLanguage('zh-TW')).toBe('zh-TW')
    expect(resolveLanguage('system', ['zh-HK'])).toBe('zh-TW')
    expect(resolveLanguage(undefined, ['fr-FR'])).toBe('en')

    Object.defineProperty(navigator, 'languages', {
      configurable: true,
      value: ['zh-MO'],
    })
    expect(resolveLanguage('system')).toBe('zh-TW')
  })

  test('creates translators with interpolation, labels, and compatibility fallbacks', () => {
    const english = createTranslator('en')
    const simplified = createTranslator('zh-CN')
    const traditional = createTranslator('zh-TW')

    expect(english('dashboard.selectedProfiles', { count: 3 })).toBe(
      '3 selected profiles',
    )
    expect(english('selectedProfiles', { count: 3 })).toBe(
      '3 selected profiles',
    )
    expect(simplified('import.revertBatch')).toBe('回滚批次')
    expect(traditional('import.revertBatch')).toBe('回滾批次')
    expect(languageLabel('system', 'en')).toBe('Follow system')
    expect(languageLabel('zh-CN', 'zh-TW')).toBe('简体中文')
    expect(languageLabel('zh-TW', 'en')).toBe('繁體中文')
    expect(languageLabel('en', 'zh-CN')).toBe('English')
    expect(english('notARealKey')).toBe('notARealKey')
    expect(localeTag('zh-TW')).toBe('zh-TW')
  })

  test('keeps trust-critical namespaces aligned across locales', () => {
    const catalog = translationCatalog()
    const englishKeys = collectLeafKeys(catalog.en)

    for (const language of ['zh-CN', 'zh-TW'] as const) {
      expect(collectLeafKeys(catalog[language])).toEqual(englishKeys)
    }

    expect(translationNamespaces).toEqual([
      'common',
      'shell',
      'navigation',
      'dashboard',
      'audit',
      'import',
      'schedule',
      'security',
      'settings',
      'platform',
    ])
  })

  test('supports a pseudo-locale for overflow-oriented testing', () => {
    expect(pseudoLocalize('Review {count} files')).toBe(
      '［Rëvïëw {count} fïlës］',
    )
  })
})
