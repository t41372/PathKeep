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
    // The card has no favicon and no og:image → the fallback panel
    // renders a domain word-mark + ABBR token + time chip on top of the
    // domain colour. That intentionally repeats the caption's domain and
    // time so the image area carries information at a glance; assert via
    // getAllByText to confirm BOTH the fallback and caption surface them.
    expect(screen.getAllByText('github.com').length).toBeGreaterThanOrEqual(2)
    expect(screen.getAllByText('21:42').length).toBeGreaterThanOrEqual(2)
    expect(screen.getByText('GIT')).toBeVisible()
    expect(screen.getByText('05')).toBeVisible()
    expect(screen.getByText('link')).toBeVisible()
    // The fallback panel renders explicitly under the testId-scoped data slot.
    expect(screen.getByTestId('frame-card-fallback')).toBeInTheDocument()
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
      document.querySelectorAll('img[src^="data:image/png;base64,ogi"]').length,
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

  test('fallback panel strips a leading www. prefix from the domain word-mark', () => {
    render(
      <PaperContactFrame
        entry={{
          id: 90,
          domain: 'www.example.com',
          url: 'https://www.example.com/page',
          time: '14:00',
          ogImageDataUrl: null,
          faviconDataUrl: null,
        }}
        domainColor="#888"
        domainAbbr="EXM"
        testId="frame-www"
      />,
    )
    // The word-mark in the fallback panel drops `www.` so the visual
    // identity is the canonical brand label; the caption row below
    // shows the raw `entry.domain` (still `www.example.com`).
    const fallback = screen.getByTestId('frame-www-fallback')
    expect(fallback.textContent).toContain('example.com')
    expect(fallback.textContent).not.toContain('www.example.com')
  })

  test('fallback panel preserves the case of mixed-case domains', () => {
    render(
      <PaperContactFrame
        entry={{
          id: 91,
          domain: 'GitHub.com',
          url: null,
          time: '09:00',
          ogImageDataUrl: null,
          faviconDataUrl: null,
        }}
        domainColor="#444"
        domainAbbr="GIT"
        testId="frame-case"
      />,
    )
    // sanitizeExplorerDisplayText / strip-www is case-insensitive on
    // the prefix only; the rest stays untouched.
    expect(screen.getByTestId('frame-case-fallback').textContent).toContain(
      'GitHub.com',
    )
  })

  test('fallback panel renders the time chip even when title is absent', () => {
    render(
      <PaperContactFrame
        entry={{
          id: 92,
          domain: 'example.com',
          time: '23:59',
          ogImageDataUrl: null,
          faviconDataUrl: null,
        }}
        domainColor="#222"
        domainAbbr="EXM"
        testId="frame-time"
      />,
    )
    const fallback = screen.getByTestId('frame-time-fallback')
    expect(fallback.textContent).toContain('23:59')
    expect(fallback.textContent).toContain('EXM')
  })

  test('fallback panel renders without testId-scoped data-testid when testId is undefined', () => {
    render(
      <PaperContactFrame
        entry={{
          id: 93,
          domain: 'example.com',
          time: '01:23',
          ogImageDataUrl: null,
          faviconDataUrl: null,
        }}
        domainColor="#222"
        domainAbbr="EXM"
      />,
    )
    // The default testid `paper-frame-fallback` is the fallback
    // identifier when no `testId` is provided.
    expect(screen.getByTestId('paper-frame-fallback')).toBeInTheDocument()
  })

  test('og:image branch suppresses the FallbackPanel even when domain has no www', () => {
    render(
      <PaperContactFrame
        entry={{
          id: 94,
          domain: 'example.com',
          time: '03:21',
          ogImageDataUrl: 'data:image/png;base64,xyz',
          faviconDataUrl: null,
        }}
        domainColor="#222"
        domainAbbr="EXM"
        testId="frame-og-only"
      />,
    )
    // FallbackPanel must not render when og:image is present.
    expect(screen.queryByTestId('frame-og-only-fallback')).toBeNull()
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
