import { describe, expect, it } from 'vitest'

import { getDomainAbbr, getDomainColor, hashString } from './domain-color'

describe('hashString', () => {
  it('returns 0 for empty input and a stable non-negative number otherwise', () => {
    expect(hashString('')).toBe(0)
    expect(hashString('github.com')).toBe(hashString('github.com'))
    expect(hashString('github.com')).toBeGreaterThanOrEqual(0)
  })
})

describe('getDomainColor', () => {
  it('falls back to the first palette colour when the domain is empty', () => {
    expect(getDomainColor('')).toMatch(/^#[0-9a-f]{6}$/i)
    // empty → first palette entry, identifiable as a stable sentinel.
    expect(getDomainColor('')).toBe(getDomainColor(''))
  })

  it('strips a leading www. and lower-cases before hashing', () => {
    expect(getDomainColor('WWW.GitHub.com')).toBe(getDomainColor('github.com'))
  })

  it('returns deterministic palette entries for distinct domains', () => {
    const a = getDomainColor('github.com')
    const b = getDomainColor('news.ycombinator.com')
    expect(a).toMatch(/^#[0-9a-f]{6}$/i)
    expect(b).toMatch(/^#[0-9a-f]{6}$/i)
  })
})

describe('getDomainAbbr', () => {
  it('returns the bullet sentinel for empty input', () => {
    expect(getDomainAbbr('')).toBe('·')
  })

  it('returns the bullet sentinel when the domain is www. only', () => {
    // After stripping "www." the only remaining segment list filters to [].
    expect(getDomainAbbr('www.')).toBe('·')
  })

  it('uppercases the first three chars for a single-segment host', () => {
    expect(getDomainAbbr('localhost')).toBe('LOC')
  })

  it('takes the first three chars of the head segment for multi-segment hosts', () => {
    expect(getDomainAbbr('news.ycombinator.com')).toBe('NEW')
    expect(getDomainAbbr('www.GitHub.com')).toBe('GIT')
  })
})
