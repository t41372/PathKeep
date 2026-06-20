import { describe, expect, it } from 'vitest'
import { yearReviewText } from './year-review-copy'
import type { ResolvedLanguage } from '../../lib/i18n'

const languages: ResolvedLanguage[] = ['en', 'zh-CN', 'zh-TW']

describe('yearReviewText', () => {
  it('returns a string for every key in every language', () => {
    const keys = [
      'heroTitle',
      'heroTitleSoFar',
      'statTotalVisits',
      'statNewDomains',
      'statDeepReads',
      'heatmapLess',
      'heatmapMore',
      'volumeHeading',
      'volumeBusiestDay',
      'volumeActiveDays',
      'podiumHeading',
      'podiumVisits',
      'researchHeading',
      'researchJourneys',
      'discoveryHeading',
      'discoveryNewSites',
      'discoveryExploratory',
      'mixHeading',
      'habitsHeading',
      'habitsDaily',
      'habitsWeekly',
      'habitsPeriodic',
      'refindHeading',
      'refindRevisits',
      'footerCta',
      'emptyTitle',
      'emptyBody',
      'loading',
      'yearPagerPrev',
      'yearPagerNext',
    ] as const

    for (const lang of languages) {
      for (const key of keys) {
        const result = yearReviewText(lang, key)
        expect(result).toBeTruthy()
        expect(typeof result).toBe('string')
      }
    }
  })

  it('interpolates {year} in heroTitle', () => {
    expect(yearReviewText('en', 'heroTitle', { year: 2025 })).toBe(
      'Your 2025 in Pages',
    )
    expect(yearReviewText('zh-CN', 'heroTitle', { year: 2025 })).toContain(
      '2025',
    )
    expect(yearReviewText('zh-TW', 'heroTitle', { year: 2025 })).toContain(
      '2025',
    )
  })

  it('interpolates {count} in podiumVisits', () => {
    expect(yearReviewText('en', 'podiumVisits', { count: 42 })).toBe(
      '42 visits',
    )
    expect(yearReviewText('zh-CN', 'podiumVisits', { count: 42 })).toContain(
      '42',
    )
  })

  it('interpolates multiple vars in volumeBusiestDay', () => {
    const result = yearReviewText('en', 'volumeBusiestDay', {
      date: '2025-07-15',
      count: 300,
    })
    expect(result).toContain('2025-07-15')
    expect(result).toContain('300')
  })

  it('interpolates {count} and {total} in volumeActiveDays', () => {
    const result = yearReviewText('en', 'volumeActiveDays', {
      count: 200,
      total: 365,
    })
    expect(result).toContain('200')
    expect(result).toContain('365')
  })

  it('returns raw template unchanged when no vars supplied', () => {
    const result = yearReviewText('en', 'heroTitle')
    // Without vars, placeholders are preserved as-is
    expect(result).toBe('Your {year} in Pages')
  })

  it('drops placeholders missing from a supplied vars object', () => {
    // vars is present (so interpolation runs) but lacks `year`, exercising the
    // "key not in vars" branch that replaces the placeholder with an empty string.
    const result = yearReviewText('en', 'heroTitle', { unrelated: 1 })
    expect(result).toBe('Your  in Pages')
  })

  it('returns text without placeholders for non-template keys', () => {
    const result = yearReviewText('en', 'loading')
    expect(result).toBe('Loading year review...')
  })
})
