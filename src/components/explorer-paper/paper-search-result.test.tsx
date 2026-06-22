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

  test('renders a star (testId-scoped) and toggles it without selecting the row', () => {
    const onSelect = vi.fn()
    const onToggle = vi.fn()
    render(
      <PaperSearchResult
        entry={makeEntry()}
        domainColor="#24292e"
        domainAbbr="GIT"
        onSelect={onSelect}
        star={{
          starred: false,
          onToggle,
          starLabel: 'Star',
          unstarLabel: 'Unstar',
        }}
        testId="result-star"
      />,
    )
    fireEvent.click(screen.getByTestId('result-star-star'))
    expect(onToggle).toHaveBeenCalledTimes(1)
    expect(onSelect).not.toHaveBeenCalled()
  })

  test('frames the enrichment excerpt by its source pill, with no match-claim caption (REACH-C3)', () => {
    render(
      <PaperSearchResult
        entry={makeEntry({
          // A pure-semantic hit: the excerpt is the page summary, which need not
          // contain the query, so the row must NOT claim "matched in" — only the
          // honest source pill frames it.
          matchReason: 'Semantic match',
          enrichmentExcerpt: 'An ergonomic async runtime for Rust',
          enrichmentSourceLabel: 'Page summary',
        })}
        domainColor="#24292e"
        domainAbbr="GIT"
        testId="result-enrichment"
      />,
    )

    const block = screen.getByTestId('result-enrichment-enrichment')
    expect(within(block).getByText('Page summary')).toBeVisible()
    // No match-claim caption is rendered anywhere on the row.
    expect(screen.queryByText('Matched in enriched content')).toBeNull()
    expect(
      within(block).getByText((content) =>
        content.includes('An ergonomic async runtime for Rust'),
      ),
    ).toBeVisible()
  })

  test('highlights the query inside the enrichment excerpt', () => {
    render(
      <PaperSearchResult
        entry={makeEntry({
          enrichmentExcerpt: 'An ergonomic async runtime for Rust',
        })}
        domainColor="#24292e"
        domainAbbr="GIT"
        query="async"
        testId="result-enrichment-mark"
      />,
    )

    const block = screen.getByTestId('result-enrichment-mark-enrichment')
    const marks = block.querySelectorAll('mark')
    expect(Array.from(marks).map((node) => node.textContent)).toEqual(['async'])
  })

  test('omits the enrichment block when the entry has no excerpt', () => {
    render(
      <PaperSearchResult
        entry={makeEntry()}
        domainColor="#24292e"
        domainAbbr="GIT"
        testId="result-no-enrichment"
      />,
    )

    expect(screen.queryByTestId('result-no-enrichment-enrichment')).toBeNull()
  })

  test('renders the enrichment excerpt with the default testId and suppresses the pill when no source label is set', () => {
    render(
      <PaperSearchResult
        entry={makeEntry({
          enrichmentExcerpt: 'An ergonomic async runtime for Rust',
        })}
        domainColor="#24292e"
        domainAbbr="GIT"
      />,
    )

    // No `testId` prop → the component falls back to its default enrichment id,
    // and with no source label only the excerpt shows (the pill is suppressed).
    const block = screen.getByTestId('paper-search-result-enrichment')
    expect(
      within(block).getByText((content) =>
        content.includes('An ergonomic async runtime for Rust'),
      ),
    ).toBeVisible()
    expect(within(block).queryByText('Page summary')).toBeNull()
  })

  test('renders the Smart match-reason caption when supplied', () => {
    render(
      <PaperSearchResult
        entry={makeEntry({ matchReason: 'Lexical + semantic match' })}
        domainColor="#24292e"
        domainAbbr="GIT"
        testId="result-reason"
      />,
    )
    expect(screen.getByTestId('result-reason-match-reason')).toHaveTextContent(
      'Lexical + semantic match',
    )
  })

  test('omits the match-reason caption when the entry has none (keyword row)', () => {
    render(
      <PaperSearchResult
        entry={makeEntry()}
        domainColor="#24292e"
        domainAbbr="GIT"
        testId="result-no-reason"
      />,
    )
    expect(screen.queryByTestId('result-no-reason-match-reason')).toBeNull()
  })

  test('renders the relevance band pill with its tone and label', () => {
    render(
      <PaperSearchResult
        entry={makeEntry({
          relevanceBand: { label: 'High confidence', tone: 'success' },
        })}
        domainColor="#24292e"
        domainAbbr="GIT"
        testId="result-band"
      />,
    )
    const band = screen.getByTestId('result-band-band')
    expect(band).toHaveTextContent('High confidence')
    expect(band).toHaveAttribute('data-tone', 'success')
  })

  test.each([
    ['warning', 'Relevant'],
    ['blocked', 'No score'],
    ['info', 'Weak match'],
  ] as const)('renders the band pill for the %s tone', (tone, label) => {
    render(
      <PaperSearchResult
        entry={makeEntry({ relevanceBand: { label, tone } })}
        domainColor="#24292e"
        domainAbbr="GIT"
        testId={`result-band-${tone}`}
      />,
    )
    expect(screen.getByTestId(`result-band-${tone}-band`)).toHaveTextContent(
      label,
    )
  })

  // P3: `scoreBand` never emits `warning`, but `bandToneClass` keeps a
  // defensive `warning` -> neutral arm so the pill never renders unstyled if an
  // external caller ever passes it. The mid `info` ("Relevant") tier shares that
  // exact neutral palette. Assert the resolved class (not just the label) so the
  // defensive mapping is genuinely covered, not merely executed.
  test.each(['warning', 'info'] as const)(
    'maps the %s tone to the neutral mid pill palette',
    (tone) => {
      render(
        <PaperSearchResult
          entry={makeEntry({ relevanceBand: { label: 'Relevant', tone } })}
          domainColor="#24292e"
          domainAbbr="GIT"
          testId={`result-neutral-${tone}`}
        />,
      )
      const band = screen.getByTestId(`result-neutral-${tone}-band`)
      // The neutral mid step: a default ink box, distinct from the accent
      // `success` pill and the faintest `blocked`/unscored treatment.
      expect(band).toHaveClass(
        'border-border-default',
        'text-ink-secondary',
        'bg-card-paper',
      )
      expect(band).not.toHaveClass('border-accent')
      expect(band).not.toHaveClass('bg-transparent')
    },
  )

  test('omits the band pill when the entry has no relevance band', () => {
    render(
      <PaperSearchResult
        entry={makeEntry()}
        domainColor="#24292e"
        domainAbbr="GIT"
        testId="result-no-band"
      />,
    )
    expect(screen.queryByTestId('result-no-band-band')).toBeNull()
  })

  test('Ask assistant fires its handler (Smart rows only) without selecting the row', () => {
    const onSelect = vi.fn()
    const onAskAssistant = vi.fn()
    const entry = makeEntry({ matchReason: 'Semantic match' })
    render(
      <PaperSearchResult
        entry={entry}
        domainColor="#24292e"
        domainAbbr="GIT"
        onSelect={onSelect}
        onAskAssistant={onAskAssistant}
        askAssistantLabel="Ask assistant"
        testId="result-ask"
      />,
    )
    fireEvent.click(screen.getByTestId('paper-search-result-ask-assistant'))
    expect(onAskAssistant).toHaveBeenCalledWith(entry)
    expect(onSelect).not.toHaveBeenCalled()
  })

  test('Ask assistant is suppressed on a row without a match reason (keyword row)', () => {
    const onAskAssistant = vi.fn()
    render(
      <PaperSearchResult
        entry={makeEntry()}
        domainColor="#24292e"
        domainAbbr="GIT"
        onAskAssistant={onAskAssistant}
        askAssistantLabel="Ask assistant"
        testId="result-no-ask"
      />,
    )
    // No matchReason → the Smart-only Ask-assistant affordance never renders.
    expect(screen.queryByTestId('paper-search-result-ask-assistant')).toBeNull()
  })
})
