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
  buildPaperSearchViewCopy,
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
    // Single-visit fallback label used by the Explorer mount.
    expect(copy.visitedLabel).toBe('Visited')
  })

  test('builds the detail-panel copy for Simplified Chinese', () => {
    const copy = buildPaperDetailPanelCopy(tFor('zh-CN'))
    expect(copy.recordEyebrow).toBe('记录')
    expect(copy.notesHeading).toBe('你的笔记')
    expect(copy.lookFurtherHeading).toBe('看得更深')
    expect(copy.visitedLabel).toBe('访问时间')
  })

  test('builds the detail-panel copy for Traditional Chinese', () => {
    const copy = buildPaperDetailPanelCopy(tFor('zh-TW'))
    expect(copy.recordEyebrow).toBe('紀錄')
    expect(copy.notesHeading).toBe('你的筆記')
    expect(copy.lookFurtherHeading).toBe('看得更深')
    expect(copy.visitedLabel).toBe('造訪時間')
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

describe('buildPaperSearchViewCopy', () => {
  test('builds Search view copy for English with nested hero and empty', () => {
    const copy = buildPaperSearchViewCopy(tFor('en'))
    expect(copy.hero.prompt).toBe('What would you like to find again?')
    expect(copy.hero.modeKeyword).toBe('Keyword')
    expect(copy.hero.modeSmart).toBe('Smart search')
    expect(copy.hero.modeHintSmartUnavailable).toContain('Settings')
    expect(copy.hero.modeSmartUnavailableAria).toBe('(unavailable)')
    expect(copy.relevance?.askAssistantLabel).toBe('Ask assistant')
    expect(copy.relevance?.rankedCount).toBe('ranked by relevance')
    expect(copy.hero.removeChipLabel).toContain('{label}')
    expect(copy.hero.advancedSyntaxHelp.title).toBe('Advanced keyword syntax')
    expect(copy.hero.advancedSyntaxHelp.siteExclude).toContain('github.com')
    expect(copy.hero.advancedSyntaxHelp.tag).toContain('tag')
    expect(copy.hero.advancedSyntaxHelp.note).toContain('note')
    // §3.3 A optimisation — the annotation chip labels resolve to
    // non-empty translated strings so the search hero can render the
    // `+ Tag` / `+ Note` add-chips alongside the inert Date / Source /
    // Domain / Visit count ones.
    expect(copy.hero.addFilterTag).toBe('+ Tag')
    expect(copy.hero.addFilterNote).toBe('+ Note')
    // Explicit-submit search gate copy resolves end-to-end.
    expect(copy.hero.inputPlaceholder).toBe('Search your archive')
    expect(copy.hero.searchButton).toBe('Search')
    expect(copy.hero.searchingButton).toBe('Searching…')
    expect(copy.hero.searchButtonAria).toBe('Search history')
    expect(copy.hero.searchingButtonAria).toBe('Searching history…')
    expect(copy.hero.submitHint).toBe('Press Enter or Search to run')
    expect(copy.hero.staleBanner).toContain('{mode}')
    expect(copy.hero.staleModeNames.keyword).toBe('Keyword')
    expect(copy.hero.staleModeNames.regex).toBe('Regex')
    expect(copy.hero.staleModeNames.smart).toBe('Smart')
    expect(copy.empty.tryAskingHeading).toBe('Try asking')
    expect(copy.empty.recentMeta).toContain('{mode}')
    expect(copy.resultsCount).toContain('{count}')
    expect(copy.resultsCount).toContain('{noun}')
    expect(copy.pageSuffixSingular).toBe('page')
    expect(copy.pageSuffixPlural).toBe('pages')
    expect(copy.noMatchesTitle).toBe('Memory is patient.')
    expect(copy.seeInContextLabel).toBe('See in context →')
    expect(copy.dayCountTemplate).toContain('{count}')
  })

  test('builds Search view copy for Simplified Chinese', () => {
    const copy = buildPaperSearchViewCopy(tFor('zh-CN'))
    expect(copy.hero.prompt).toBe('你想再找回什么？')
    expect(copy.hero.modeKeyword).toBe('关键词')
    expect(copy.empty.tryAskingHeading).toBe('试着这样问')
    expect(copy.pageSuffixSingular).toBe('页')
    expect(copy.seeInContextLabel).toBe('回到当天 →')
    expect(copy.hero.addFilterTag).toBe('+ 标签')
    expect(copy.hero.addFilterNote).toBe('+ 笔记')
  })

  test('builds Search view copy for Traditional Chinese', () => {
    const copy = buildPaperSearchViewCopy(tFor('zh-TW'))
    expect(copy.hero.prompt).toBe('你想再找回什麼？')
    expect(copy.hero.modeKeyword).toBe('關鍵字')
    expect(copy.empty.recentHeading).toBe('最近搜尋')
    expect(copy.noMatchesTitle).toBe('記憶需要時間。')
    expect(copy.hero.addFilterTag).toBe('+ 標籤')
    expect(copy.hero.addFilterNote).toBe('+ 筆記')
  })

  test('Search view copy has no missing-key leakage across locales', () => {
    for (const language of ['en', 'zh-CN', 'zh-TW'] as const) {
      const copy = buildPaperSearchViewCopy(tFor(language))
      // Strip the nested `advancedSyntaxHelp` + `staleModeNames` objects out
      // before flattening so the leakage check only walks strings. Their string
      // members are folded back into `all` below. `advancedSyntaxHelp` lives in
      // a different namespace (`explorer.advancedSearchHelp*`).
      const { advancedSyntaxHelp, staleModeNames, ...heroStrings } = copy.hero
      const all: string[] = [
        ...Object.values(heroStrings),
        ...Object.values(advancedSyntaxHelp),
        ...Object.values(staleModeNames),
        ...Object.values(copy.empty),
        copy.resultsCount,
        copy.resultsRange,
        copy.pageSuffixSingular,
        copy.pageSuffixPlural,
        copy.noMatchesTitle,
        copy.noMatchesBody,
        copy.seeInContextLabel,
        copy.dayCountTemplate,
      ]
      for (const value of all) {
        expect(value).not.toMatch(/explorer\.paperSearchView\./)
        expect(value).not.toMatch(/explorer\.advancedSearchHelp/)
      }
    }
  })
})
