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

    expect(screen.getByText('https://docs.rs/sqlx')).toBeVisible()
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

    expect(screen.getByText('https://docs.rs')).toBeVisible()
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
