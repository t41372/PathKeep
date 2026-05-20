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
    const markTexts = Array.from(marks).map((node) =>
      node.textContent?.toLowerCase(),
    )
    expect(markTexts).toContain('rust')
    expect(markTexts).toContain('async')
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

  test('survives a malformed query gracefully', () => {
    expect(() =>
      render(
        <PaperSearchResult
          entry={makeEntry()}
          domainColor="#24292e"
          domainAbbr="GIT"
          // Unclosed character class — would throw if not handled.
          query="["
          testId="result-bad-query"
        />,
      ),
    ).not.toThrow()
  })
})
