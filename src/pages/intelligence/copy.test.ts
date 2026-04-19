import { describe, expect, test } from 'vitest'

import {
  formatDomainPagePath,
  intelligenceCategoryLabel,
  intelligenceText,
} from './copy'

describe('intelligence display copy helpers', () => {
  test('falls back to shipped archive-wide copy when a translator returns the raw namespace key', () => {
    const brokenTranslator = (key: string) => `intelligence.${key}`

    expect(
      intelligenceText('zh-TW', brokenTranslator, 'archiveWideBadge'),
    ).toBe('全部封存統計')
    expect(
      intelligenceText('zh-CN', brokenTranslator, 'archiveWideBody'),
    ).toContain('整份存档')
    expect(
      intelligenceText('en', brokenTranslator, 'externalOutputsReviewBody'),
    ).toContain('Settings')
  })

  test('falls back to a localized community label when the category key is missing', () => {
    const brokenTranslator = (key: string) => `intelligence.${key}`

    expect(
      intelligenceCategoryLabel('zh-CN', brokenTranslator, 'community'),
    ).toBe('社区')
    expect(
      intelligenceCategoryLabel('zh-TW', brokenTranslator, 'community'),
    ).toBe('社群')
    expect(intelligenceCategoryLabel('en', brokenTranslator, 'community')).toBe(
      'Community',
    )
  })

  test('decodes domain page paths for visible UI text', () => {
    expect(
      formatDomainPagePath(
        '/wiki/%E5%93%88%E5%B8%83%E6%96%AF%E5%A0%A1%E5%90%9B%E4%B8%BB%E5%9C%8B',
      ),
    ).toBe('/wiki/哈布斯堡君主國')
  })
})
