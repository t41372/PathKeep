/**
 * Tests for PaperSearchEmpty — suggestion cards + recent searches.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import {
  PaperSearchEmpty,
  type PaperSearchEmptyCopy,
  type PaperSearchRecent,
  type PaperSearchSuggestion,
} from './paper-search-empty'

const COPY: PaperSearchEmptyCopy = {
  tryAskingHeading: 'Try asking',
  recentHeading: 'Recent',
  recentMeta: '{mode} · {count} results · {when}',
  recentMetaBrief: '{mode}',
  // Deliberately not the raw tokens: the row must render these display names,
  // never the internal `keyword` / `regex` / `smart` values.
  modeNames: { keyword: 'Keyword', regex: 'Regex', smart: 'Smart' },
  footer: 'Search is local. Nothing leaves your machine.',
  smartPrompt: 'Ask in plain language — that article about Rust async.',
}

const SUGGESTIONS: PaperSearchSuggestion[] = [
  {
    id: 's1',
    cue: 'Just ask',
    text: 'What was that paper about transformer architecture?',
    hint: 'Semantic recall · ~12 results',
  },
  {
    id: 's2',
    cue: 'By domain',
    text: 'All my visits to docs.rs this year',
    hint: '178 results',
  },
]

const RECENT: PaperSearchRecent[] = [
  {
    id: 'r1',
    q: 'tokio scheduler',
    mode: 'keyword',
    count: 89,
    when: 'yesterday',
  },
  {
    id: 'r2',
    q: 'gaussian splatting',
    // `smart`, not the retired `semantic`. The prop type used to say
    // `'keyword' | 'regex' | 'semantic'`, which matched nothing else on the
    // surface — the row was unassignable from real data.
    mode: 'smart',
    count: 27,
    when: 'last week',
  },
]

/** A row persisted before the count/timestamp were recorded. */
const LEGACY_RECENT: PaperSearchRecent[] = [
  { id: 'legacy', q: 'wal checkpoint', mode: 'regex' },
]

describe('PaperSearchEmpty', () => {
  test('renders both suggestion cards and recent searches', () => {
    render(
      <PaperSearchEmpty
        suggestions={SUGGESTIONS}
        recent={RECENT}
        copy={COPY}
        testId="empty"
      />,
    )

    expect(screen.getByText('Try asking')).toBeVisible()
    expect(screen.getByText('Recent')).toBeVisible()
    expect(
      screen.getByText('What was that paper about transformer architecture?'),
    ).toBeVisible()
    expect(screen.getByText('tokio scheduler')).toBeVisible()
  })

  test('8c: shows the Smart natural-language prompt only when smartActive', () => {
    const { rerender } = render(
      <PaperSearchEmpty
        suggestions={SUGGESTIONS}
        smartActive
        copy={COPY}
        testId="empty"
      />,
    )

    const prompt = screen.getByTestId('paper-search-empty-smart-prompt')
    expect(prompt).toBeVisible()
    expect(prompt).toHaveTextContent(COPY.smartPrompt)

    // Keyword/regex empty states never surface the Smart prompt.
    rerender(
      <PaperSearchEmpty
        suggestions={SUGGESTIONS}
        smartActive={false}
        copy={COPY}
        testId="empty"
      />,
    )
    expect(screen.queryByTestId('paper-search-empty-smart-prompt')).toBeNull()
  })

  test('clicking a suggestion fires onPickSuggestion with the entry', () => {
    const onPickSuggestion = vi.fn()
    render(
      <PaperSearchEmpty
        suggestions={SUGGESTIONS}
        copy={COPY}
        onPickSuggestion={onPickSuggestion}
      />,
    )

    fireEvent.click(screen.getByTestId('paper-search-suggestion-s1'))
    expect(onPickSuggestion).toHaveBeenCalledWith(SUGGESTIONS[0])
  })

  test('clicking a recent entry fires onRunRecent with the entry', () => {
    const onRunRecent = vi.fn()
    render(
      <PaperSearchEmpty
        recent={RECENT}
        copy={COPY}
        onRunRecent={onRunRecent}
      />,
    )

    fireEvent.click(screen.getByTestId('paper-search-recent-r2'))
    expect(onRunRecent).toHaveBeenCalledWith(RECENT[1])
  })

  test('suggestion + recent buttons are disabled without their handlers', () => {
    render(
      <PaperSearchEmpty
        suggestions={SUGGESTIONS}
        recent={RECENT}
        copy={COPY}
      />,
    )

    expect(
      screen.getByTestId<HTMLButtonElement>('paper-search-suggestion-s1')
        .disabled,
    ).toBe(true)
    expect(
      screen.getByTestId<HTMLButtonElement>('paper-search-recent-r1').disabled,
    ).toBe(true)
  })

  test('interpolates the LOCALIZED mode name plus {count} {when} into the recent-meta string', () => {
    render(
      <PaperSearchEmpty recent={RECENT} copy={COPY} onRunRecent={() => {}} />,
    )

    expect(screen.getByText('Keyword · 89 results · yesterday')).toBeVisible()
    expect(screen.getByText('Smart · 27 results · last week')).toBeVisible()
    // The internal token must never reach the screen.
    expect(screen.queryByText(/\bkeyword\b/)).toBeNull()
  })

  test('falls back to the brief caption for entries with no recorded count', () => {
    render(
      <PaperSearchEmpty
        recent={LEGACY_RECENT}
        copy={COPY}
        onRunRecent={() => {}}
      />,
    )

    expect(screen.getByText('wal checkpoint')).toBeVisible()
    expect(screen.getByText('Regex')).toBeVisible()
    // Never "undefined results".
    expect(screen.queryByText(/undefined/)).toBeNull()
  })

  test('skips the suggestion section when no suggestions are supplied', () => {
    render(<PaperSearchEmpty recent={RECENT} copy={COPY} />)
    expect(screen.queryByText('Try asking')).toBeNull()
    expect(screen.getByText('Recent')).toBeVisible()
  })

  test('skips the recent section when no recent entries are supplied', () => {
    render(<PaperSearchEmpty suggestions={SUGGESTIONS} copy={COPY} />)
    expect(screen.queryByText('Recent')).toBeNull()
    expect(screen.getByText('Try asking')).toBeVisible()
  })

  test('always renders the quiet footer line', () => {
    render(<PaperSearchEmpty copy={COPY} testId="empty-footer-only" />)
    expect(
      screen.getByText('Search is local. Nothing leaves your machine.'),
    ).toBeVisible()
  })

  test('omits the suggestion hint when not provided', () => {
    render(
      <PaperSearchEmpty
        suggestions={[{ id: 'plain', cue: 'Cue', text: 'Plain example' }]}
        copy={COPY}
        onPickSuggestion={() => {}}
      />,
    )

    expect(screen.getByText('Cue')).toBeVisible()
    expect(screen.getByText('Plain example')).toBeVisible()
    expect(screen.queryByText('results')).toBeNull()
  })
})
