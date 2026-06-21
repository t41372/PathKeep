/**
 * Tests for the Starred hub view.
 */

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import {
  PaperStarredView,
  type PaperStarredViewCopy,
} from './paper-starred-view'
import type { StarListItem } from '../../lib/backend-client'

const COPY: PaperStarredViewCopy = {
  eyebrow: 'STARRED',
  title: 'Starred',
  description: 'Pages and sources you keep close.',
  groupPages: 'Pages',
  groupSources: 'Sources',
  sortLabel: 'Sort',
  sortRecent: 'Recently starred',
  sortRevisited: 'Most revisited',
  loading: 'Gathering…',
  emptyTitle: 'Nothing starred yet',
  emptyBody: 'Star a page to keep it here.',
  emptyCta: 'Browse your history →',
  visitCountTemplate: '{count}×',
  starAction: 'Star',
  unstarAction: 'Unstar',
  statusStarred: 'Starred',
  statusUnstarred: 'Unstarred',
}

function page(overrides: Partial<StarListItem> = {}): StarListItem {
  return {
    entityKind: 'url',
    entityKey: 'https://example.com/post',
    starredAt: '2026-04-01T00:00:00Z',
    domain: 'example.com',
    title: 'A post',
    visitCount: 5,
    ...overrides,
  }
}

function source(overrides: Partial<StarListItem> = {}): StarListItem {
  return {
    entityKind: 'domain',
    entityKey: 'example.com',
    starredAt: '2026-04-01T00:00:00Z',
    domain: 'example.com',
    title: '',
    visitCount: 9,
    ...overrides,
  }
}

describe('PaperStarredView', () => {
  test('renders the loading skeleton while loading', () => {
    render(
      <PaperStarredView
        items={[]}
        loading
        sort="recently_starred"
        onSortChange={() => {}}
        onToggleStar={() => {}}
        copy={COPY}
      />,
    )
    expect(screen.getByTestId('paper-starred-skeleton')).toBeInTheDocument()
  })

  test('renders the empty state when there are no items', () => {
    render(
      <PaperStarredView
        items={[]}
        loading={false}
        sort="recently_starred"
        onSortChange={() => {}}
        onToggleStar={() => {}}
        copy={COPY}
      />,
    )
    expect(screen.getByTestId('paper-starred-empty')).toBeInTheDocument()
    expect(screen.getByText('Star a page to keep it here.')).toBeVisible()
  })

  test('empty state renders a Browse-history CTA that fires onBrowseHistory', () => {
    const onBrowseHistory = vi.fn()
    render(
      <PaperStarredView
        items={[]}
        loading={false}
        sort="recently_starred"
        onSortChange={() => {}}
        onToggleStar={() => {}}
        onBrowseHistory={onBrowseHistory}
        copy={COPY}
      />,
    )
    fireEvent.click(screen.getByTestId('paper-starred-empty-cta'))
    expect(onBrowseHistory).toHaveBeenCalledTimes(1)
  })

  test('empty state omits the CTA when no onBrowseHistory is wired', () => {
    render(
      <PaperStarredView
        items={[]}
        loading={false}
        sort="recently_starred"
        onSortChange={() => {}}
        onToggleStar={() => {}}
        copy={COPY}
      />,
    )
    expect(screen.queryByTestId('paper-starred-empty-cta')).toBeNull()
  })

  test('source chip un-star uses the shared StarToggle (aria-pressed semantics)', () => {
    render(
      <PaperStarredView
        items={[source()]}
        loading={false}
        sort="recently_starred"
        onSortChange={() => {}}
        onToggleStar={() => {}}
        copy={COPY}
      />,
    )
    expect(
      screen.getByTestId('paper-starred-source-star-example.com'),
    ).toHaveAttribute('aria-pressed', 'true')
  })

  test('groups pages and sources and renders both', () => {
    render(
      <PaperStarredView
        items={[page(), source()]}
        loading={false}
        sort="recently_starred"
        onSortChange={() => {}}
        onToggleStar={() => {}}
        copy={COPY}
      />,
    )
    expect(screen.getByText('Pages')).toBeVisible()
    expect(screen.getByText('Sources')).toBeVisible()
    expect(
      screen.getByTestId('paper-starred-page-https://example.com/post'),
    ).toBeInTheDocument()
    expect(
      screen.getByTestId('paper-starred-source-example.com'),
    ).toBeInTheDocument()
  })

  test('selecting a page card and a source chip calls onSelect', () => {
    const onSelect = vi.fn()
    render(
      <PaperStarredView
        items={[page(), source()]}
        loading={false}
        sort="recently_starred"
        onSortChange={() => {}}
        onSelect={onSelect}
        onToggleStar={() => {}}
        copy={COPY}
      />,
    )
    fireEvent.click(
      screen.getByTestId('paper-starred-page-https://example.com/post'),
    )
    fireEvent.click(screen.getByTestId('paper-starred-source-example.com'))
    expect(onSelect).toHaveBeenCalledTimes(2)
  })

  test('un-starring a page card and a source chip calls onToggleStar', () => {
    const onToggleStar = vi.fn()
    render(
      <PaperStarredView
        items={[page(), source()]}
        loading={false}
        sort="recently_starred"
        onSortChange={() => {}}
        onToggleStar={onToggleStar}
        copy={COPY}
      />,
    )
    fireEvent.click(
      screen.getByTestId('paper-starred-page-https://example.com/post-star'),
    )
    fireEvent.click(screen.getByTestId('paper-starred-source-star-example.com'))
    expect(onToggleStar).toHaveBeenCalledTimes(2)
  })

  test('changing the sort select calls onSortChange', () => {
    const onSortChange = vi.fn()
    render(
      <PaperStarredView
        items={[page()]}
        loading={false}
        sort="recently_starred"
        onSortChange={onSortChange}
        onToggleStar={() => {}}
        copy={COPY}
      />,
    )
    fireEvent.change(screen.getByTestId('paper-starred-sort'), {
      target: { value: 'most_revisited' },
    })
    expect(onSortChange).toHaveBeenCalledWith('most_revisited')
  })

  test('renders only the Sources group when there are no page stars', () => {
    render(
      <PaperStarredView
        items={[source()]}
        loading={false}
        sort="recently_starred"
        onSortChange={() => {}}
        onToggleStar={() => {}}
        copy={COPY}
      />,
    )
    expect(screen.queryByText('Pages')).toBeNull()
    expect(screen.getByText('Sources')).toBeVisible()
  })

  test('renders only the Pages group (and keys color off the URL when domain is empty)', () => {
    render(
      <PaperStarredView
        items={[page({ domain: '', entityKey: 'https://nodomain.test/p' })]}
        loading={false}
        sort="recently_starred"
        onSortChange={() => {}}
        onToggleStar={() => {}}
        copy={COPY}
      />,
    )
    expect(screen.getByText('Pages')).toBeVisible()
    expect(screen.queryByText('Sources')).toBeNull()
    expect(
      screen.getByTestId('paper-starred-page-https://nodomain.test/p'),
    ).toBeInTheDocument()
  })

  test('renders cards/sources without onSelect (inert click is safe)', () => {
    render(
      <PaperStarredView
        items={[page(), source()]}
        loading={false}
        sort="recently_starred"
        onSortChange={() => {}}
        onToggleStar={() => {}}
        copy={COPY}
      />,
    )
    // No onSelect provided: the source chip button is disabled, the card click
    // is a no-op. Neither throws.
    expect(
      screen.getByTestId<HTMLButtonElement>('paper-starred-source-example.com')
        .disabled,
    ).toBe(true)
    expect(() =>
      fireEvent.click(
        screen.getByTestId('paper-starred-page-https://example.com/post'),
      ),
    ).not.toThrow()
  })

  test('source chip hides its visit count when zero, page falls back to its key', () => {
    render(
      <PaperStarredView
        items={[
          page({ title: '', entityKey: 'https://x.test/y', visitCount: 0 }),
          source({ visitCount: 0 }),
        ]}
        loading={false}
        sort="recently_starred"
        onSortChange={() => {}}
        onToggleStar={() => {}}
        copy={COPY}
      />,
    )
    // Page card with no title shows its URL key as the title.
    expect(
      screen.getByTestId('paper-starred-page-https://x.test/y'),
    ).toBeInTheDocument()
    // Source chip text is just the domain (no "9×") when visitCount is 0.
    const chip = screen.getByTestId('paper-starred-source-example.com')
    expect(chip.textContent).toBe('example.com')
  })
})
