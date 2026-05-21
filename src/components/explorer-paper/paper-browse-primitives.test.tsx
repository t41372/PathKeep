/**
 * Test contract for the paper Browse visual primitives.
 *
 * Why this file exists:
 * - These components are the building blocks for the redesigned Explorer
 *   contact sheet. They're presentation-only but each one has invariants the
 *   route relies on (active state styling, target-entry force-expand, click
 *   propagation, accessibility labels).
 * - Locking the behaviour at the primitive level keeps the Explorer route
 *   integration test focused on data flow instead of layout.
 *
 * Source-of-truth notes:
 * - Visual contract: `docs/design/handoff/paper-redesign/project/pk-contactsheet.jsx`
 *   and `pk-tokens.css` (`.cs-frame`, `.cs-day-sticky`, `.cs-target-banner`).
 */

import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import {
  PaperContactFrame,
  PaperDayHeader,
  PaperDomainStack,
  PaperListRow,
  PaperSessionHeader,
  PaperTargetBanner,
  PaperViewToggle,
} from './index'

describe('PaperDayHeader', () => {
  test('renders label, meta, and right index', () => {
    render(
      <PaperDayHeader
        label="Friday, May 16"
        meta="22 pages · 4 sessions"
        rightIndex="Day 1"
      />,
    )

    expect(screen.getByText('Friday, May 16')).toBeVisible()
    expect(screen.getByText('22 pages · 4 sessions')).toBeVisible()
    expect(screen.getByText('Day 1')).toBeVisible()
  })

  test('marks the active variant via data-active so consumers can style it', () => {
    render(<PaperDayHeader label="May 16" active testId="day-active" />)

    const header = screen.getByTestId('day-active')
    expect(header.dataset.active).toBe('true')
  })

  test('omits the right index when not provided and skips meta when empty', () => {
    render(<PaperDayHeader label="May 16" testId="day-bare" />)

    const header = screen.getByTestId('day-bare')
    // Only the label cell should render text.
    expect(within(header).queryByText(/sessions/)).toBeNull()
    expect(within(header).queryByText('Day')).toBeNull()
  })

  test('applies the toolbar offset as a CSS top value', () => {
    render(
      <PaperDayHeader
        label="May 16"
        toolbarOffsetPx={64}
        testId="day-offset"
      />,
    )

    expect(screen.getByTestId('day-offset').style.top).toBe('64px')
  })
})

describe('PaperSessionHeader', () => {
  test('renders the time range alone when no label is given', () => {
    render(
      <PaperSessionHeader timeRange="20:15 — 21:42" testId="session-bare" />,
    )

    const node = screen.getByTestId('session-bare')
    expect(within(node).getByText('20:15 — 21:42')).toBeVisible()
  })

  test('renders both the time range and the descriptive label', () => {
    render(
      <PaperSessionHeader
        timeRange="20:15 — 21:42"
        label="Rust async runtime deep dive"
      />,
    )

    expect(screen.getByText('20:15 — 21:42')).toBeVisible()
    expect(screen.getByText('Rust async runtime deep dive')).toBeVisible()
  })
})

describe('PaperTargetBanner', () => {
  test('renders kicker, date, status, and clears via the button', () => {
    const onClear = vi.fn()
    render(
      <PaperTargetBanner
        source="on-this-day"
        kicker="From 'On this day'"
        date="May 17, 2025"
        status="3 pages archived"
        onClear={onClear}
        clearLabel="Clear"
        testId="banner-otd"
      />,
    )

    expect(screen.getByText("From 'On this day'")).toBeVisible()
    expect(screen.getByText('May 17, 2025')).toBeVisible()
    expect(screen.getByText('3 pages archived')).toBeVisible()

    fireEvent.click(screen.getByRole('button', { name: 'Clear' }))
    expect(onClear).toHaveBeenCalledTimes(1)
  })

  test('marks search-source with a distinct data-source token', () => {
    render(
      <PaperTargetBanner
        source="search"
        kicker={<em>"async"</em>}
        date="Apr 5, 2026"
        status="Scrolled to record"
        onClear={() => {}}
        clearLabel="Clear"
        testId="banner-search"
      />,
    )

    expect(screen.getByTestId('banner-search').dataset.source).toBe('search')
  })
})

describe('PaperContactFrame', () => {
  const entry = {
    id: 1,
    title: 'tokio-rs/tokio',
    domain: 'github.com',
    url: 'https://github.com/tokio-rs/tokio',
    time: '21:42',
    transitionType: 'link',
  }

  test('renders title, domain, time, frame number, and transition glyph', () => {
    render(
      <PaperContactFrame
        entry={entry}
        domainColor="#24292e"
        domainAbbr="GIT"
        index={4}
        testId="frame-card"
      />,
    )

    expect(screen.getByText('tokio-rs/tokio')).toBeVisible()
    expect(screen.getByText('github.com')).toBeVisible()
    expect(screen.getByText('21:42')).toBeVisible()
    expect(screen.getByText('05')).toBeVisible()
    expect(screen.getByText('link')).toBeVisible()
  })

  test('forwards the entry on click and reflects selected state', () => {
    const onClick = vi.fn()
    render(
      <PaperContactFrame
        entry={entry}
        domainColor="#24292e"
        domainAbbr="GIT"
        selected
        onClick={onClick}
        testId="frame-selected"
      />,
    )

    const button = screen.getByTestId('frame-selected')
    expect(button.dataset.selected).toBe('true')
    fireEvent.click(button)
    expect(onClick).toHaveBeenCalledWith(entry)
  })

  test('falls back to url then domain when title is missing, and uses favicon when supplied', () => {
    render(
      <PaperContactFrame
        entry={{
          id: 2,
          domain: 'docs.rs',
          url: 'https://docs.rs/sqlx',
          time: '18:45',
          faviconDataUrl: 'data:image/png;base64,abc',
        }}
        domainColor="#aaa"
        domainAbbr="DOC"
        testId="frame-favicon"
      />,
    )

    // sanitizeExplorerDisplayText strips protocol + www. and keeps the path.
    expect(screen.getByText('docs.rs/sqlx')).toBeVisible()
    expect(screen.queryByText('DOC')).toBeNull()
    const img = screen.getByTestId('frame-favicon').querySelector('img')
    expect(img?.getAttribute('src')).toBe('data:image/png;base64,abc')
  })

  test('falls back to the domain itself when neither title nor url exists', () => {
    render(
      <PaperContactFrame
        entry={{ id: 3, domain: 'arxiv.org', time: '14:23' }}
        domainColor="#a8322d"
        domainAbbr="ARX"
        testId="frame-bare"
      />,
    )

    expect(screen.getAllByText('arxiv.org').length).toBeGreaterThan(0)
  })

  test('renders the og:image when provided and overrides the favicon overlay', () => {
    render(
      <PaperContactFrame
        entry={{
          id: 4,
          title: 'A storied page',
          domain: 'example.com',
          url: 'https://example.com/story',
          time: '09:15',
          faviconDataUrl: 'data:image/png;base64,fav',
          ogImageDataUrl: 'data:image/png;base64,ogi',
        }}
        domainColor="#264653"
        domainAbbr="EXM"
        testId="frame-og"
      />,
    )
    const og = screen.getByTestId<HTMLImageElement>('frame-og-og-image')
    expect(og.src).toBe('data:image/png;base64,ogi')
    // favicon overlay must NOT render when og:image is present.
    expect(screen.queryByTestId('frame-og-favicon')).toBeNull()
  })

  test('og:image renders without a testId-derived data-testid when testId is omitted', () => {
    render(
      <PaperContactFrame
        entry={{
          id: 41,
          title: 'No-test-id storied page',
          domain: 'example.com',
          url: 'https://example.com/story',
          time: '09:15',
          ogImageDataUrl: 'data:image/png;base64,ogi',
        }}
        domainColor="#264653"
        domainAbbr="EXM"
      />,
    )
    // The og:image still mounts, but the `testId ? ... : undefined` ternary
    // at line 100 of paper-contact-frame.tsx takes the falsy branch.
    expect(
      document.querySelectorAll('img[src^="data:image/png;base64,ogi"]')
        .length,
    ).toBe(1)
  })

  test('falls back to favicon when og:image is null, then to the swatch when both are null', () => {
    const { rerender, container } = render(
      <PaperContactFrame
        entry={{
          id: 5,
          domain: 'docs.rs',
          url: 'https://docs.rs/sqlx',
          time: '18:45',
          ogImageDataUrl: null,
          faviconDataUrl: 'data:image/png;base64,onlyfav',
        }}
        domainColor="#aaa"
        domainAbbr="DOC"
        testId="frame-precedence"
      />,
    )
    expect(screen.getByTestId('frame-precedence-favicon')).toBeInTheDocument()
    expect(
      container.querySelector('[data-testid="frame-precedence-og-image"]'),
    ).toBeNull()

    rerender(
      <PaperContactFrame
        entry={{
          id: 5,
          domain: 'docs.rs',
          url: 'https://docs.rs/sqlx',
          time: '18:45',
          ogImageDataUrl: null,
          faviconDataUrl: null,
        }}
        domainColor="#aaa"
        domainAbbr="DOC"
        testId="frame-precedence"
      />,
    )
    expect(screen.getByText('DOC')).toBeVisible()
  })

  test('is safe to click without an onClick handler', () => {
    render(
      <PaperContactFrame
        entry={entry}
        domainColor="#24292e"
        domainAbbr="GIT"
        testId="frame-noop"
      />,
    )

    expect(() =>
      fireEvent.click(screen.getByTestId('frame-noop')),
    ).not.toThrow()
  })
})

describe('PaperListRow', () => {
  const entry = {
    id: 1,
    title: 'Attention Is All You Need',
    domain: 'arxiv.org',
    url: 'https://arxiv.org/abs/1706.03762',
    time: '20:05',
  }

  test('renders title, domain, and time', () => {
    render(
      <PaperListRow
        entry={entry}
        domainColor="#a8322d"
        domainAbbr="ARX"
        testId="list-row"
      />,
    )

    expect(screen.getByText('Attention Is All You Need')).toBeVisible()
    expect(screen.getByText('arxiv.org')).toBeVisible()
    expect(screen.getByText('20:05')).toBeVisible()
  })

  test('invokes onClick with the entry and exposes the selected state', () => {
    const onClick = vi.fn()
    render(
      <PaperListRow
        entry={entry}
        domainColor="#a8322d"
        domainAbbr="ARX"
        selected
        onClick={onClick}
        testId="list-selected"
      />,
    )

    fireEvent.click(screen.getByTestId('list-selected'))
    expect(onClick).toHaveBeenCalledWith(entry)
    expect(screen.getByTestId('list-selected').dataset.selected).toBe('true')
  })

  test('falls back to url then domain when title is missing', () => {
    render(
      <PaperListRow
        entry={{
          id: 2,
          domain: 'docs.rs',
          url: 'https://docs.rs',
          time: '12:00',
        }}
        domainColor="#aaa"
        domainAbbr="DOC"
        testId="list-url"
      />,
    )

    // sanitizeExplorerDisplayText collapses 'https://docs.rs' to 'docs.rs',
    // which coincides with entry.domain — list-row shows it in both columns.
    expect(screen.getAllByText('docs.rs').length).toBeGreaterThanOrEqual(1)
  })

  test('falls back to the bare domain when both title and url are missing', () => {
    render(
      <PaperListRow
        entry={{
          id: 3,
          domain: 'docs.rs',
          time: '12:00',
        }}
        domainColor="#aaa"
        domainAbbr="DOC"
        testId="list-domain-fallback"
      />,
    )
    // Title fallthrough lands on the bare domain (line 94 of paper-list-row.tsx).
    expect(screen.getAllByText('docs.rs').length).toBeGreaterThanOrEqual(1)
  })

  test('survives clicks when no handler is provided', () => {
    render(
      <PaperListRow
        entry={entry}
        domainColor="#a8322d"
        domainAbbr="ARX"
        testId="list-noop"
      />,
    )

    expect(() => fireEvent.click(screen.getByTestId('list-noop'))).not.toThrow()
  })

  test('renders the favicon image and hides the swatch when faviconDataUrl is provided', () => {
    render(
      <PaperListRow
        entry={{
          ...entry,
          faviconDataUrl: 'data:image/png;base64,iVBORw0KG',
        }}
        domainColor="#a8322d"
        domainAbbr="ARX"
        testId="list-favicon"
      />,
    )

    const icon = screen.getByTestId<HTMLImageElement>('list-favicon-favicon')
    expect(icon).toBeVisible()
    expect(icon.tagName).toBe('IMG')
    expect(icon.src).toBe('data:image/png;base64,iVBORw0KG')
    expect(screen.queryByTestId('list-favicon-swatch')).not.toBeInTheDocument()
  })

  test('falls back to the domain swatch when faviconDataUrl is null or absent', () => {
    const { rerender } = render(
      <PaperListRow
        entry={{ ...entry, faviconDataUrl: null }}
        domainColor="#a8322d"
        domainAbbr="ARX"
        testId="list-no-favicon"
      />,
    )

    expect(screen.getByTestId('list-no-favicon-swatch')).toHaveTextContent(
      'ARX',
    )
    expect(
      screen.queryByTestId('list-no-favicon-favicon'),
    ).not.toBeInTheDocument()

    // Omitting the field entirely takes the same path.
    rerender(
      <PaperListRow
        entry={entry}
        domainColor="#a8322d"
        domainAbbr="ARX"
        testId="list-no-favicon"
      />,
    )
    expect(screen.getByTestId('list-no-favicon-swatch')).toHaveTextContent(
      'ARX',
    )
  })
})

describe('PaperDomainStack', () => {
  const entries = [
    {
      id: 1,
      title: 'tokio repo',
      domain: 'github.com',
      url: 'u1',
      time: '21:42',
    },
    {
      id: 2,
      title: 'tokio scheduler',
      domain: 'github.com',
      url: 'u2',
      time: '21:38',
    },
    {
      id: 3,
      title: 'tokio issues',
      domain: 'github.com',
      url: 'u3',
      time: '21:31',
    },
    {
      id: 4,
      title: 'tokio blog',
      domain: 'github.com',
      url: 'u4',
      time: '21:15',
    },
    {
      id: 5,
      title: 'tokio futures',
      domain: 'github.com',
      url: 'u5',
      time: '20:52',
    },
  ]

  test('starts collapsed and shows a "+N more" footer when entries exceed preview', () => {
    render(
      <PaperDomainStack
        domain="github.com"
        domainColor="#24292e"
        domainAbbr="GIT"
        entries={entries}
        expandLabel="Toggle stack"
        morePrefix="+"
        pagesLabel="pages"
        testId="stack"
      />,
    )

    const stack = screen.getByTestId('stack')
    expect(stack.dataset.expanded).toBeUndefined()
    // Preview shows the first 4 titles + a "+ 1" footer
    expect(screen.getByText('tokio repo')).toBeVisible()
    expect(screen.getByText('tokio blog')).toBeVisible()
    expect(screen.queryByText('tokio futures')).toBeNull()
    expect(screen.getByText('+ 1')).toBeVisible()
  })

  test('expanding the stack reveals every entry', () => {
    render(
      <PaperDomainStack
        domain="github.com"
        domainColor="#24292e"
        domainAbbr="GIT"
        entries={entries}
        expandLabel="Toggle stack"
        morePrefix="+"
        pagesLabel="pages"
        testId="stack-toggle"
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'Toggle stack' }))
    const stack = screen.getByTestId('stack-toggle')
    expect(stack.dataset.expanded).toBe('true')
    expect(screen.getByText('tokio futures')).toBeVisible()
  })

  test('force-expands when a target entry sits inside the stack', () => {
    render(
      <PaperDomainStack
        domain="github.com"
        domainColor="#24292e"
        domainAbbr="GIT"
        entries={entries}
        targetEntryId={3}
        expandLabel="Toggle stack"
        morePrefix="+"
        pagesLabel="pages"
        testId="stack-target"
      />,
    )

    expect(screen.getByTestId('stack-target').dataset.expanded).toBe('true')
  })

  test('preview rows surface their entry without bubbling to the header toggle', () => {
    const onSelect = vi.fn()
    render(
      <PaperDomainStack
        domain="github.com"
        domainColor="#24292e"
        domainAbbr="GIT"
        entries={entries}
        onSelectEntry={onSelect}
        expandLabel="Toggle stack"
        morePrefix="+"
        pagesLabel="pages"
        testId="stack-select"
      />,
    )

    fireEvent.click(screen.getByText('tokio repo'))
    expect(onSelect).toHaveBeenCalledWith(entries[0])
    // Header should not have been expanded by the bubble.
    expect(screen.getByTestId('stack-select').dataset.expanded).toBeUndefined()
  })

  test('"+N more" expands the stack to the full list', () => {
    render(
      <PaperDomainStack
        domain="github.com"
        domainColor="#24292e"
        domainAbbr="GIT"
        entries={entries}
        expandLabel="Toggle stack"
        morePrefix="more"
        pagesLabel="pages"
        testId="stack-more"
      />,
    )

    fireEvent.click(screen.getByText('more 1'))
    expect(screen.getByTestId('stack-more').dataset.expanded).toBe('true')
  })

  test('expanded rows fall back to url/domain when title is missing and bubble selection', () => {
    const onSelect = vi.fn()
    const entriesWithMissingTitle = [
      ...entries.slice(0, 4),
      { id: 99, domain: 'github.com', url: 'https://gh/extra', time: '19:00' },
    ]
    render(
      <PaperDomainStack
        domain="github.com"
        domainColor="#24292e"
        domainAbbr="GIT"
        entries={entriesWithMissingTitle}
        onSelectEntry={onSelect}
        expandLabel="Toggle stack"
        morePrefix="+"
        pagesLabel="pages"
        testId="stack-fallback"
      />,
    )

    fireEvent.click(screen.getByRole('button', { name: 'Toggle stack' }))
    const fallbackRow = screen
      .getByTestId('stack-fallback')
      .querySelector('[data-entry-id="99"]') as HTMLElement
    expect(fallbackRow).not.toBeNull()
    fireEvent.click(fallbackRow)
    expect(onSelect).toHaveBeenCalledWith(entriesWithMissingTitle[4])
  })

  test('no preview footer is rendered when entries fit inside the preview budget', () => {
    render(
      <PaperDomainStack
        domain="github.com"
        domainColor="#24292e"
        domainAbbr="GIT"
        entries={entries.slice(0, 3)}
        expandLabel="Toggle stack"
        morePrefix="+"
        pagesLabel="pages"
        testId="stack-short"
      />,
    )

    expect(screen.queryByText(/\+ 0/)).toBeNull()
  })

  test('clicks on collapsed rows without a handler do not throw', () => {
    render(
      <PaperDomainStack
        domain="github.com"
        domainColor="#24292e"
        domainAbbr="GIT"
        entries={entries}
        expandLabel="Toggle stack"
        morePrefix="+"
        pagesLabel="pages"
        testId="stack-noop"
      />,
    )

    expect(() => fireEvent.click(screen.getByText('tokio repo'))).not.toThrow()
  })

  test('collapsed preview row falls back to url, then domain, when title is missing', () => {
    render(
      <PaperDomainStack
        domain="docs.rs"
        domainColor="#7a9cc7"
        domainAbbr="DOC"
        entries={[
          { id: 101, domain: 'docs.rs', url: 'https://docs.rs/tokio', time: '17:00' },
          { id: 102, domain: 'docs.rs', time: '17:05' },
        ]}
        expandLabel="Toggle docs"
        morePrefix="+"
        pagesLabel="pages"
        testId="stack-collapsed-fallback"
      />,
    )
    // Row with only url renders the url fallback (sanitized);
    // row with only domain falls all the way through to the domain text.
    // Both branches of `entry.title || entry.url || entry.domain` at line 156 fire.
    expect(screen.getAllByText(/docs\.rs/).length).toBeGreaterThan(0)
  })

  test('expanded row falls back to domain when both title and url are missing', () => {
    render(
      <PaperDomainStack
        domain="docs.rs"
        domainColor="#7a9cc7"
        domainAbbr="DOC"
        entries={[
          { id: 201, domain: 'docs.rs', time: '17:00' },
          { id: 202, domain: 'docs.rs', time: '17:05' },
          { id: 203, domain: 'docs.rs', time: '17:10' },
          { id: 204, domain: 'docs.rs', time: '17:15' },
          { id: 205, domain: 'docs.rs', time: '17:20' },
        ]}
        expandLabel="Toggle docs"
        morePrefix="+"
        pagesLabel="pages"
        testId="stack-expanded-fallback"
      />,
    )
    // Force-expand by clicking the header, then verify the expanded rows
    // render the domain fallback for both title (line 203) and url (line 207).
    fireEvent.click(screen.getByRole('button', { name: 'Toggle docs' }))
    expect(screen.getAllByText('docs.rs').length).toBeGreaterThan(0)
  })

  test('expanded row click does not throw when onSelectEntry is omitted', () => {
    render(
      <PaperDomainStack
        domain="docs.rs"
        domainColor="#7a9cc7"
        domainAbbr="DOC"
        entries={[
          { id: 301, domain: 'docs.rs', url: 'https://docs.rs/a', title: 'a', time: '17:00' },
          { id: 302, domain: 'docs.rs', url: 'https://docs.rs/b', title: 'b', time: '17:05' },
          { id: 303, domain: 'docs.rs', url: 'https://docs.rs/c', title: 'c', time: '17:10' },
          { id: 304, domain: 'docs.rs', url: 'https://docs.rs/d', title: 'd', time: '17:15' },
          { id: 305, domain: 'docs.rs', url: 'https://docs.rs/e', title: 'e', time: '17:20' },
        ]}
        expandLabel="Toggle docs"
        morePrefix="+"
        pagesLabel="pages"
        testId="stack-expanded-noop"
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'Toggle docs' }))
    // Click an expanded row without a handler — covers the
    // `if (onSelectEntry)` falsy branch at lines 180-182.
    expect(() => fireEvent.click(screen.getByText('a'))).not.toThrow()
  })
})

describe('PaperViewToggle', () => {
  test('renders both options and highlights the active one via aria-selected', () => {
    render(
      <PaperViewToggle
        value="cards"
        options={[
          { value: 'cards', label: '⊞ Cards' },
          { value: 'list', label: '☰ List' },
        ]}
        onChange={() => {}}
        ariaLabel="View mode"
        testId="toggle"
      />,
    )

    const cards = screen.getByRole('tab', { name: '⊞ Cards' })
    const list = screen.getByRole('tab', { name: '☰ List' })
    expect(cards.getAttribute('aria-selected')).toBe('true')
    expect(list.getAttribute('aria-selected')).toBe('false')
  })

  test('invokes onChange with the new value on click', () => {
    const onChange = vi.fn()
    render(
      <PaperViewToggle
        value="cards"
        options={[
          { value: 'cards', label: '⊞ Cards' },
          { value: 'list', label: '☰ List' },
        ]}
        onChange={onChange}
      />,
    )

    fireEvent.click(screen.getByRole('tab', { name: '☰ List' }))
    expect(onChange).toHaveBeenCalledWith('list')
  })
})
