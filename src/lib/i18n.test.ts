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

function collectLeafEntries(
  node: Record<string, string | Record<string, unknown>>,
  prefix = '',
  entries: Array<{ key: string; value: string }> = [],
) {
  for (const [key, value] of Object.entries(node)) {
    const next = prefix ? `${prefix}.${key}` : key
    if (typeof value === 'string') {
      entries.push({ key: next, value })
      continue
    }
    collectLeafEntries(
      value as Record<string, string | Record<string, unknown>>,
      next,
      entries,
    )
  }

  return entries.sort((left, right) => left.key.localeCompare(right.key))
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
    expect(resolveLanguage('pirate' as never)).toBe('en')

    Object.defineProperty(navigator, 'languages', {
      configurable: true,
      value: ['zh-MO'],
    })
    expect(resolveLanguage('system')).toBe('zh-TW')

    Object.defineProperty(navigator, 'languages', {
      configurable: true,
      value: [],
    })
    Object.defineProperty(navigator, 'language', {
      configurable: true,
      value: 'en-GB',
    })
    expect(detectSystemLanguage()).toBe('en')

    const navigatorDescriptor = Object.getOwnPropertyDescriptor(
      globalThis,
      'navigator',
    )
    Object.defineProperty(globalThis, 'navigator', {
      configurable: true,
      value: undefined,
    })
    expect(detectSystemLanguage()).toBe('en')
    if (navigatorDescriptor) {
      Object.defineProperty(globalThis, 'navigator', navigatorDescriptor)
    } else {
      Reflect.deleteProperty(globalThis, 'navigator')
    }
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
    expect(
      english('dashboard.selectedProfiles', { count: null as never }),
    ).toBe('{count} selected profiles')
    expect(
      createTranslator('pirate' as never)('dashboard.selectedProfiles', {
        count: 2,
      }),
    ).toBe('2 selected profiles')
    expect(
      createTranslator('en', true)('dashboard.selectedProfiles', {
        count: 2,
      }),
    ).toBe('［2 sëlëctëd prôfïlës］')
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
      'recovery',
    ])
  })

  test('supports a pseudo-locale for overflow-oriented testing', () => {
    expect(pseudoLocalize('Review {count} files')).toBe(
      '［Rëvïëw {count} fïlës］',
    )
    expect(pseudoLocalize('@@__9__@@')).toBe('［］')
  })

  test('keeps onboarding browser support copy aligned with the validated matrix', () => {
    const english = createNamespaceTranslator('en', 'onboarding')
    const simplified = createNamespaceTranslator('zh-CN', 'onboarding')
    const traditional = createNamespaceTranslator('zh-TW', 'onboarding')

    expect(english('featureBackupDesc')).toContain('Google Chrome')
    expect(english('featureBackupDesc')).toContain('Microsoft Edge')
    expect(english('featureBackupDesc')).toContain('Firefox')
    expect(english('featureBackupDesc')).toContain('ChatGPT Atlas')
    expect(english('featureBackupDesc')).toContain('Perplexity Comet')
    expect(english('featureBackupDesc')).toContain('Safari')
    expect(english('featureBackupDesc')).not.toContain('Brave')
    expect(english('firefoxSafariInfo')).not.toContain('fully supported')
    expect(english('firefoxSafariInfo')).toContain('macOS-only support paths')

    expect(simplified('featureBackupDesc')).toContain('Google Chrome')
    expect(simplified('featureBackupDesc')).toContain('Microsoft Edge')
    expect(simplified('featureBackupDesc')).toContain('Firefox')
    expect(simplified('featureBackupDesc')).toContain('ChatGPT Atlas')
    expect(simplified('featureBackupDesc')).toContain('Perplexity Comet')
    expect(simplified('featureBackupDesc')).toContain('Safari')
    expect(simplified('featureBackupDesc')).not.toContain('Brave')
    expect(simplified('firefoxSafariInfo')).toContain('macOS 路径')

    expect(traditional('featureBackupDesc')).toContain('Google Chrome')
    expect(traditional('featureBackupDesc')).toContain('Microsoft Edge')
    expect(traditional('featureBackupDesc')).toContain('Firefox')
    expect(traditional('featureBackupDesc')).toContain('ChatGPT Atlas')
    expect(traditional('featureBackupDesc')).toContain('Perplexity Comet')
    expect(traditional('featureBackupDesc')).toContain('Safari')
    expect(traditional('featureBackupDesc')).not.toContain('Brave')
    expect(traditional('firefoxSafariInfo')).toContain('macOS 路徑')
  })

  test('blocks known raw English backend/debug phrases from Chinese UI catalogs', () => {
    const catalog = translationCatalog()
    const blocked = [
      /\bFull Disk Access\b/i,
      /Safari History\.db is not readable yet/i,
      /Grant Full Disk Access/i,
      /\barchive facts\b/i,
      /\bcanonical archive run\b/i,
      /\bshell state\b/i,
      /\bcopying\s+\/Users\//i,
      /\bprofile\b/i,
      /\badapter\b/i,
      /\bappend-only\b/i,
      /^[a-z]+(?:\.[a-z][a-z0-9]*){1,}$/i,
      /\bapp data\b/i,
    ]

    for (const language of ['zh-CN', 'zh-TW'] as const) {
      const offenders = collectLeafEntries(catalog[language]).filter(
        (entry) => {
          const visibleValue = entry.value.replace(/\{[a-zA-Z0-9_]+\}/g, '')
          return blocked.some((pattern) => pattern.test(visibleValue))
        },
      )

      expect(offenders).toEqual([])
    }
  })

  test('resolves onboarding platform and browser-engine labels instead of leaking keys', () => {
    for (const language of ['en', 'zh-CN', 'zh-TW'] as const) {
      const onboarding = createNamespaceTranslator(language, 'onboarding')

      expect(onboarding('platform.macosLabel')).toBe('macOS')
      expect(onboarding('platform.windowsLabel')).toBe('Windows')
      expect(onboarding('platform.linuxLabel')).toBe('Linux')
      expect(onboarding('browserEngineChromium')).toBe('Chromium')
      expect(onboarding('browserEngineSafari')).toBe('Safari')
      expect(onboarding('browserEngineFirefox')).toBe('Firefox')
    }
  })

  test('keeps intelligence archive-wide and category copy available in every shipping locale', () => {
    for (const language of ['en', 'zh-CN', 'zh-TW'] as const) {
      const intelligence = createNamespaceTranslator(language, 'intelligence')

      expect(intelligence('archiveWideBadge')).not.toBe(
        'intelligence.archiveWideBadge',
      )
      expect(intelligence('archiveWideBody')).not.toBe(
        'intelligence.archiveWideBody',
      )
      expect(intelligence('externalOutputsReviewBody')).not.toBe(
        'intelligence.externalOutputsReviewBody',
      )
      expect(intelligence('category_community')).not.toBe(
        'intelligence.category_community',
      )
    }
  })
})
