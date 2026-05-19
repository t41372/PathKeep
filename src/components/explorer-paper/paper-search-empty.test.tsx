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
  footer: 'Search is local. Nothing leaves your machine.',
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
    mode: 'semantic',
    count: 27,
    when: 'last week',
  },
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

  test('interpolates {mode} {count} {when} into the recent-meta string', () => {
    render(
      <PaperSearchEmpty recent={RECENT} copy={COPY} onRunRecent={() => {}} />,
    )

    expect(screen.getByText('keyword · 89 results · yesterday')).toBeVisible()
    expect(screen.getByText('semantic · 27 results · last week')).toBeVisible()
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
