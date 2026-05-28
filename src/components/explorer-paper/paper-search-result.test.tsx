/**
 * Tests for PaperSearchResult — single search result row with optional
 * snippet, query highlighting, and "See in context" jump.
 */

import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import {
  PaperSearchResult,
  type PaperSearchResultEntry,
} from './paper-search-result'

function makeEntry(
  overrides: Partial<PaperSearchResultEntry> = {},
): PaperSearchResultEntry {
  return {
    id: 1,
    title: 'tokio-rs/tokio: rust async runtime',
    url: 'https://github.com/tokio-rs/tokio',
    domain: 'github.com',
    time: '21:42',
    transitionType: 'link',
    ...overrides,
  }
}

describe('PaperSearchResult', () => {
  test('renders title, domain · url, time, and transition type', () => {
    render(
      <PaperSearchResult
        entry={makeEntry()}
        domainColor="#24292e"
        domainAbbr="GIT"
        testId="result"
      />,
    )

    expect(screen.getByText('tokio-rs/tokio: rust async runtime')).toBeVisible()
    expect(
      screen.getByText('github.com · github.com/tokio-rs/tokio'),
    ).toBeVisible()
    expect(screen.getByText('21:42')).toBeVisible()
    expect(screen.getByText('link')).toBeVisible()
  })

  test('omits the transition badge when the entry does not provide one', () => {
    render(
      <PaperSearchResult
        entry={makeEntry({ transitionType: undefined })}
        domainColor="#24292e"
        domainAbbr="GIT"
        testId="result-no-transition"
      />,
    )

    expect(screen.getByTestId('result-no-transition')).toBeVisible()
    expect(screen.queryByText('link')).not.toBeInTheDocument()
  })

  test('wraps matching tokens of the query in <mark> tags', () => {
    render(
      <PaperSearchResult
        entry={makeEntry()}
        domainColor="#24292e"
        domainAbbr="GIT"
        query="rust async"
        testId="result-mark"
      />,
    )

    const row = screen.getByTestId('result-mark')
    const marks = row.querySelectorAll('mark')
    // Title is 'tokio-rs/tokio: rust async runtime'; 'rust' and 'async' each
    // appear exactly once, so the highlight must produce exactly two <mark>s.
    // Pin the count so a regression that double-wraps or wraps adjacent
    // punctuation is caught.
    expect(marks).toHaveLength(2)
    const markTexts = Array.from(marks).map((node) =>
      node.textContent?.toLowerCase(),
    )
    expect(markTexts).toEqual(['rust', 'async'])
  })

  test('clicking the row surfaces the entry via onSelect', () => {
    const onSelect = vi.fn()
    const entry = makeEntry()
    render(
      <PaperSearchResult
        entry={entry}
        domainColor="#24292e"
        domainAbbr="GIT"
        onSelect={onSelect}
        testId="result-click"
      />,
    )

    fireEvent.click(screen.getByTestId('result-click'))
    expect(onSelect).toHaveBeenCalledWith(entry)
  })

  test('Enter and Space activate the row via keyboard', () => {
    const onSelect = vi.fn()
    render(
      <PaperSearchResult
        entry={makeEntry()}
        domainColor="#24292e"
        domainAbbr="GIT"
        onSelect={onSelect}
        testId="result-key"
      />,
    )

    const row = screen.getByTestId('result-key')
    fireEvent.keyDown(row, { key: 'Enter' })
    fireEvent.keyDown(row, { key: ' ' })
    expect(onSelect).toHaveBeenCalledTimes(2)
  })

  test('ignores unrelated keys so keyboard navigation does not select rows accidentally', () => {
    const onSelect = vi.fn()
    render(
      <PaperSearchResult
        entry={makeEntry()}
        domainColor="#24292e"
        domainAbbr="GIT"
        onSelect={onSelect}
        testId="result-arrow-key"
      />,
    )

    fireEvent.keyDown(screen.getByTestId('result-arrow-key'), {
      key: 'ArrowDown',
    })
    expect(onSelect).not.toHaveBeenCalled()
  })

  test('renders the snippet block when supplied', () => {
    render(
      <PaperSearchResult
        entry={makeEntry({
          snippet: 'A short tour of the scheduler — work stealing makes…',
        })}
        domainColor="#24292e"
        domainAbbr="GIT"
        testId="result-snippet"
      />,
    )

    const row = screen.getByTestId('result-snippet')
    expect(
      within(row).getByText((content) =>
        content.includes('A short tour of the scheduler'),
      ),
    ).toBeVisible()
  })

  test('"See in context" button fires its handler without bubbling to onSelect', () => {
    const onSelect = vi.fn()
    const onSeeInContext = vi.fn()
    const entry = makeEntry()
    render(
      <PaperSearchResult
        entry={entry}
        domainColor="#24292e"
        domainAbbr="GIT"
        onSelect={onSelect}
        onSeeInContext={onSeeInContext}
        seeInContextLabel="See in context"
        testId="result-jump"
      />,
    )

    fireEvent.click(screen.getByTestId('paper-search-result-see-in-context'))
    expect(onSeeInContext).toHaveBeenCalledWith(entry)
    expect(onSelect).not.toHaveBeenCalled()
  })

  test('omits the See-in-context button when no handler is provided', () => {
    render(
      <PaperSearchResult
        entry={makeEntry()}
        domainColor="#24292e"
        domainAbbr="GIT"
        testId="result-no-jump"
      />,
    )

    expect(
      screen.queryByTestId('paper-search-result-see-in-context'),
    ).toBeNull()
  })

  test('falls back from an empty title to url and then domain for the visible result heading', () => {
    const { rerender } = render(
      <PaperSearchResult
        entry={makeEntry({ title: '', url: 'https://example.com/docs' })}
        domainColor="#24292e"
        domainAbbr="EX"
        testId="result-url-title"
      />,
    )
    expect(screen.getByText('example.com/docs')).toBeVisible()

    rerender(
      <PaperSearchResult
        entry={makeEntry({ title: '', url: '', domain: 'example.com' })}
        domainColor="#24292e"
        domainAbbr="EX"
        testId="result-domain-title"
      />,
    )
    expect(screen.getByText('example.com')).toBeVisible()
  })

  test('falls back gracefully when query is empty', () => {
    render(
      <PaperSearchResult
        entry={makeEntry()}
        domainColor="#24292e"
        domainAbbr="GIT"
        query=""
        testId="result-empty-query"
      />,
    )

    expect(
      screen.getByTestId('result-empty-query').querySelectorAll('mark').length,
    ).toBe(0)
  })

  test('escapes regex metacharacters in the query so an unclosed bracket renders verbatim with zero matches', () => {
    expect(() =>
      render(
        <PaperSearchResult
          entry={makeEntry()}
          domainColor="#24292e"
          domainAbbr="GIT"
          // `[` is a regex metachar; without escaping `new RegExp('([)', 'gi')`
          // would throw. The escape pipeline turns it into a literal `[`,
          // which the title does not contain, so the assertion below pins
          // the contract: zero marks AND the row still renders.
          query="["
          testId="result-bad-query"
        />,
      ),
    ).not.toThrow()
    const row = screen.getByTestId('result-bad-query')
    expect(row.querySelectorAll('mark')).toHaveLength(0)
  })

  test('returns the title verbatim when the regex compile throws (defensive fallback)', () => {
    // Force `new RegExp` to throw to exercise the try/catch fallback. This
    // path is unreachable via user input today because the escape pipeline
    // catches every metachar, but the fallback is the docstring's contract
    // and a future tweak to the escape regex could expose it.
    const realRegExp = globalThis.RegExp
    let calls = 0
    // @ts-expect-error - intentional reassignment for the test.
    globalThis.RegExp = function FakeRegExp(...args: [string, string?]) {
      calls += 1
      if (calls === 1) throw new SyntaxError('forced for test')
      return new realRegExp(...args)
    }
    try {
      expect(() =>
        render(
          <PaperSearchResult
            entry={makeEntry()}
            domainColor="#24292e"
            domainAbbr="GIT"
            query="rust"
            testId="result-regex-throws"
          />,
        ),
      ).not.toThrow()
      const row = screen.getByTestId('result-regex-throws')
      expect(row.querySelectorAll('mark')).toHaveLength(0)
    } finally {
      globalThis.RegExp = realRegExp
    }
  })
})
