/**
 * Tests for buildPaperExplorerCopy.
 *
 * Why this file exists:
 * - The Explorer route hands an i18n namespace translator to the paper view
 *   via this builder. A missing key, a renamed key, or a typo lands here
 *   first so the route doesn't have to chase translator returns string by
 *   string.
 *
 * We deliberately test against the real catalog so renames in either side
 * surface immediately. The runtime translator falls back to the key name
 * when a key is missing, so an assertion that the value differs from the
 * key proves the entry exists in the catalog.
 */

import { describe, expect, test } from 'vitest'
import { createNamespaceTranslator } from '@/lib/i18n/catalog/catalog-runtime'
import {
  buildPaperDetailPanelCopy,
  buildPaperExplorerCopy,
  buildPaperIntelligenceCopy,
} from './paper-explorer-copy'

function tFor(language: 'en' | 'zh-CN' | 'zh-TW') {
  return createNamespaceTranslator(language, 'explorer')
}

describe('buildPaperExplorerCopy', () => {
  test('builds a complete copy bundle for English', () => {
    const copy = buildPaperExplorerCopy(tFor('en'))
    expect(copy.contactSheet.view).toBe('View')
    expect(copy.contactSheet.cards).toBe('Cards')
    expect(copy.contactSheet.list).toBe('List')
    expect(copy.contactSheet.dayMeta).toContain('{count}')
    expect(copy.dayNav.today).toBe('Today')
    expect(copy.relative.yesterday).toBe('yesterday')
    expect(copy.calendar.months[0]).toBe('January')
    expect(copy.calendar.months[11]).toBe('December')
    expect(copy.calendar.dowLabels).toHaveLength(7)
    expect(copy.yearRailTitle).toContain('{year}')
  })

  test('builds a complete copy bundle for Simplified Chinese', () => {
    const copy = buildPaperExplorerCopy(tFor('zh-CN'))
    expect(copy.contactSheet.view).toBe('视图')
    expect(copy.contactSheet.empty).toBe('还没有内容。记忆需要时间。')
    expect(copy.dayNav.today).toBe('今天')
    expect(copy.calendar.months[4]).toBe('五月')
    expect(copy.calendar.oneYearAgo).toBe('1 年前')
  })

  test('builds a complete copy bundle for Traditional Chinese', () => {
    const copy = buildPaperExplorerCopy(tFor('zh-TW'))
    expect(copy.contactSheet.view).toBe('檢視')
    expect(copy.contactSheet.cards).toBe('卡片')
    expect(copy.calendar.months[0]).toBe('一月')
    expect(copy.calendar.pagesArchived).toContain('{count}')
    expect(copy.calendar.boundsMeta).toContain('{firstYear}')
  })

  test('every key in the bundle resolves to a non-key value (catalog parity guard)', () => {
    for (const language of ['en', 'zh-CN', 'zh-TW'] as const) {
      const copy = buildPaperExplorerCopy(tFor(language))
      const all: string[] = [
        copy.contactSheet.view,
        copy.contactSheet.cards,
        copy.contactSheet.list,
        copy.contactSheet.dayMeta,
        copy.contactSheet.dayIndex,
        copy.contactSheet.clearTarget,
        copy.contactSheet.expandStack,
        copy.contactSheet.moreInStack,
        copy.contactSheet.pagesLabel,
        copy.contactSheet.empty,
        copy.dayNav.prev,
        copy.dayNav.next,
        copy.dayNav.today,
        copy.dayNav.openCalendar,
        copy.relative.today,
        copy.relative.yesterday,
        copy.relative.daysAgo,
        copy.relative.weeksAgo,
        copy.relative.monthsAgo,
        copy.relative.yearsAgo,
        copy.calendar.prevMonth,
        copy.calendar.nextMonth,
        ...copy.calendar.months,
        ...copy.calendar.dowLabels,
        copy.calendar.today,
        copy.calendar.oneYearAgo,
        copy.calendar.pagesArchived,
        copy.calendar.monthSummary,
        copy.calendar.boundsMeta,
        copy.yearRailTitle,
      ]
      // When a key is missing, the translator returns the key itself
      // (e.g. "explorer.paperBrowse.contactSheetView"). Any value still
      // containing the namespace prefix means catalog parity has slipped.
      for (const value of all) {
        expect(value).not.toMatch(/explorer\.paperBrowse\./)
      }
    }
  })
})

describe('buildPaperDetailPanelCopy', () => {
  test('builds the detail-panel copy for English', () => {
    const copy = buildPaperDetailPanelCopy(tFor('en'))
    expect(copy.recordEyebrow).toBe('Record')
    expect(copy.openAction).toBe('Open')
    expect(copy.notesHeading).toBe('Your notes')
    expect(copy.allOfDomain).toContain('{domain}')
    expect(copy.visitCountSuffix).toContain('{count}')
  })

  test('builds the detail-panel copy for Simplified Chinese', () => {
    const copy = buildPaperDetailPanelCopy(tFor('zh-CN'))
    expect(copy.recordEyebrow).toBe('记录')
    expect(copy.notesHeading).toBe('你的笔记')
    expect(copy.lookFurtherHeading).toBe('看得更深')
  })

  test('builds the detail-panel copy for Traditional Chinese', () => {
    const copy = buildPaperDetailPanelCopy(tFor('zh-TW'))
    expect(copy.recordEyebrow).toBe('紀錄')
    expect(copy.notesHeading).toBe('你的筆記')
    expect(copy.lookFurtherHeading).toBe('看得更深')
  })

  test('detail copy has no missing-key leakage across locales', () => {
    for (const language of ['en', 'zh-CN', 'zh-TW'] as const) {
      const copy = buildPaperDetailPanelCopy(tFor(language))
      for (const value of Object.values(copy)) {
        expect(value).not.toMatch(/explorer\.paperBrowse\./)
      }
    }
  })
})

describe('buildPaperIntelligenceCopy', () => {
  test('builds Intelligence view copy for English with default summary fallback', () => {
    const copy = buildPaperIntelligenceCopy(tFor('en'))
    expect(copy.topicsTitle).toBe('Topics, over the last 30 days')
    expect(copy.topicsRangeBadge).toContain('30D')
    expect(copy.domainsTitle).toBe('Where you spent your time')
    expect(copy.sessionsTitle).toBe('Recent sessions')
    expect(copy.threadsTitle).toBe('Active threads')
    expect(copy.refindTitle).toBe('Refind candidates')
    expect(copy.topicsSummary).toContain('Topic clustering needs local LLM')
  })

  test('honours an LLM-supplied summary override', () => {
    const copy = buildPaperIntelligenceCopy(tFor('en'), {
      topicsSummary: 'This week was dominated by Rust internals.',
    })
    expect(copy.topicsSummary).toBe(
      'This week was dominated by Rust internals.',
    )
  })

  test('builds Intelligence copy for Simplified Chinese', () => {
    const copy = buildPaperIntelligenceCopy(tFor('zh-CN'))
    expect(copy.topicsTitle).toBe('近 30 天的主题')
    expect(copy.domainsTitle).toBe('你的时间去了哪里')
    expect(copy.refindTitle).toBe('常回看候选')
  })

  test('builds Intelligence copy for Traditional Chinese', () => {
    const copy = buildPaperIntelligenceCopy(tFor('zh-TW'))
    expect(copy.topicsTitle).toBe('近 30 天的主題')
    expect(copy.threadsTitle).toBe('活躍線索')
  })

  test('Intelligence copy has no missing-key leakage across locales', () => {
    for (const language of ['en', 'zh-CN', 'zh-TW'] as const) {
      const copy = buildPaperIntelligenceCopy(tFor(language))
      const stringValues = Object.values(copy).filter(
        (value): value is string => typeof value === 'string',
      )
      for (const value of stringValues) {
        expect(value).not.toMatch(/explorer\.paperIntelligence\./)
      }
    }
  })
})
