import { describe, expect, test } from 'vitest'

import {
  createTranslator,
  detectSystemLanguage,
  languageLabel,
  localeTag,
  resolveLanguage,
} from './i18n'

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

    Object.defineProperty(navigator, 'languages', {
      configurable: true,
      value: [],
    })
    Object.defineProperty(navigator, 'language', {
      configurable: true,
      value: 'zh-CN',
    })
    expect(detectSystemLanguage()).toBe('zh-CN')

    const originalNavigator = globalThis.navigator
    Object.defineProperty(globalThis, 'navigator', {
      configurable: true,
      value: undefined,
    })
    expect(detectSystemLanguage()).toBe('en')
    Object.defineProperty(globalThis, 'navigator', {
      configurable: true,
      value: originalNavigator,
    })
  })

  test('creates translators with interpolation and translated labels', () => {
    const english = createTranslator('en')
    const simplified = createTranslator('zh-CN')
    const traditional = createTranslator('zh-TW')
    const fallback = createTranslator('pirate' as never)

    expect(english('profilesDetected', { count: 3 })).toBe('3 profiles')
    expect(english('profilesDetected', { hours: 72 })).toBe('{count} profiles')
    expect(english('profilesDetected')).toBe('{count} profiles')
    expect(simplified('revertBatch')).toBe('回滚批次')
    expect(languageLabel('system', 'en')).toBe('Follow system')
    expect(languageLabel('zh-CN', 'zh-TW')).toBe('简体中文')
    expect(languageLabel('zh-TW', 'en')).toBe('繁體中文')
    expect(languageLabel('en', 'zh-CN')).toBe('English')
    expect(traditional('revertBatch')).toBe('回滾批次')
    expect(fallback('english')).toBe('English')
    expect(fallback('notARealKey' as never)).toBe('notARealKey')
    expect(localeTag('zh-TW')).toBe('zh-TW')
  })
})
