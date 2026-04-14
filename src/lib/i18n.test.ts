/**
 * This test file protects the front-end helper and contract logic in I18n.
 *
 * Why this file exists:
 * - Pure helpers are where we keep UI policy testable without booting the whole shell.
 * - When these tests fail, they usually point at a contract drift that would otherwise show up as subtle route regressions.
 *
 * Main declarations:
 * - `collectLeafKeys`
 *
 * Source-of-truth notes:
 * - Helper behavior should stay aligned with the same design, feature, and architecture docs that guide the UI surfaces consuming it.
 * - Prefer focused behavioral assertions over snapshotting implementation detail.
 */

import { describe, expect, test } from 'vitest'

import {
  createNamespaceTranslator,
  createTranslator,
  detectSystemLanguage,
  languageLabel,
  localeTag,
  pseudoLocalize,
  resolveLanguage,
  translationCatalog,
  translationNamespaces,
} from './i18n'

/**
 * Explains how collect leaf keys works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
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
    expect(simplified('import.revertBatch')).toBe('撤销导入')
    expect(traditional('import.revertBatch')).toBe('復原匯入')
    expect(languageLabel('system', 'en')).toBe('Follow system')
    expect(languageLabel('zh-CN', 'zh-TW')).toBe('简体中文')
    expect(languageLabel('zh-TW', 'en')).toBe('繁體中文')
    expect(languageLabel('en', 'zh-CN')).toBe('English')
    expect(english('notARealKey')).toBe('notARealKey')
    expect(localeTag('zh-TW')).toBe('zh-TW')
  })

  test('keeps trust-critical namespaces aligned across locales', () => {
    /**
     * Stores the translation catalog that powers PathKeep's shipped i18n surface.
     *
     * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
     */
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
      'explorer',
      'assistant',
      'insights',
      'intelligence',
      'jobs',
      'platform',
      'onboarding',
    ])
  })

  test('supports a pseudo-locale for overflow-oriented testing', () => {
    expect(pseudoLocalize('Review {count} files')).toBe(
      '［Rëvïëw {count} fïlës］',
    )
  })

  test('keeps onboarding browser support copy aligned with the validated matrix', () => {
    const english = createNamespaceTranslator('en', 'onboarding')
    const simplified = createNamespaceTranslator('zh-CN', 'onboarding')
    const traditional = createNamespaceTranslator('zh-TW', 'onboarding')

    expect(english('featureBackupDesc')).toContain('Google Chrome')
    expect(english('featureBackupDesc')).toContain('Safari')
    expect(english('featureBackupDesc')).not.toContain('Edge')
    expect(english('featureBackupDesc')).not.toContain('Brave')
    expect(english('firefoxSafariInfo')).not.toContain('fully supported')
    expect(english('firefoxSafariInfo')).toContain('public support commitments')

    expect(simplified('featureBackupDesc')).toContain('Google Chrome')
    expect(simplified('featureBackupDesc')).toContain('Safari')
    expect(simplified('featureBackupDesc')).not.toContain('Edge')
    expect(simplified('firefoxSafariInfo')).toContain('公开支持承诺')

    expect(traditional('featureBackupDesc')).toContain('Google Chrome')
    expect(traditional('featureBackupDesc')).toContain('Safari')
    expect(traditional('featureBackupDesc')).not.toContain('Edge')
    expect(traditional('firefoxSafariInfo')).toContain('公開支援承諾')
  })
})
