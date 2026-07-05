/**
 * Tests for PaperEnrichedContent — the detail-panel "Enriched content" section.
 *
 * Covers every honest state (loading skeleton, disabled, error, empty, ready
 * body with topics, ready-but-failed status), plus the Fetch-now PME button's
 * enabled / disabled / pending / error affordances.
 */

import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, test, vi } from 'vitest'
import {
  PaperEnrichedContent,
  type PaperEnrichedContentCopy,
  type PaperEnrichedContentState,
} from './paper-enriched-content'
import type { EnrichmentView } from './paper-enriched-content-helpers'

const copy: PaperEnrichedContentCopy = {
  heading: 'Enriched content',
  loading: 'Loading enriched content…',
  empty: 'No enriched content for this page yet.',
  disabled: 'Turn on site content fetching in Settings to enrich this page.',
  error: 'Could not load enriched content.',
  fetchedAt: 'Fetched {when}',
  sourceGithub: 'GitHub',
  sourceGeneric: 'Page summary',
  sourceUnknown: 'Source',
  topicsLabel: 'Topics',
  statusEmpty: 'Nothing readable was found on this page.',
  statusBlocked: 'This site is blocked in your settings.',
  statusError: 'The fetch failed. PathKeep will retry later.',
  statusLogin: 'This page needs a sign-in, so it was not fetched.',
  statusUnsupported: 'This page is not a fetchable type.',
  statusRateLimited: 'The site is rate-limiting requests.',
  fetchNowAction: 'Fetch now',
  fetchNowFetching: 'Fetching…',
  fetchNowQueued: 'Queued. This page will be enriched shortly.',
  fetchNowDisabledHint:
    'Fetching is off. Turn on site content in Settings first.',
  fetchNowError: 'Could not start the fetch. Try again.',
}

function view(overrides: Partial<EnrichmentView> = {}): EnrichmentView {
  return {
    sourceKind: 'github',
    ok: true,
    statusKind: 'success',
    fetchedAt: '2026-06-21T00:00:00Z',
    topics: [],
    ...overrides,
  }
}

function renderSection(
  state: PaperEnrichedContentState,
  extra: Partial<
    Pick<
      Parameters<typeof PaperEnrichedContent>[0],
      'fetchEnabled' | 'fetchPending' | 'fetchError' | 'onFetchNow'
    >
  > = {},
) {
  const onFetchNow = extra.onFetchNow ?? vi.fn()
  render(
    <PaperEnrichedContent
      state={state}
      copy={copy}
      fetchEnabled={extra.fetchEnabled ?? true}
      fetchPending={extra.fetchPending ?? false}
      fetchError={extra.fetchError ?? false}
      onFetchNow={onFetchNow}
      testId="enriched"
    />,
  )
  return { onFetchNow }
}

describe('PaperEnrichedContent', () => {
  test('renders the heading and a skeleton while loading', () => {
    renderSection({ status: 'loading' })
    expect(screen.getByText('Enriched content')).toBeVisible()
    expect(screen.getByTestId('enriched-skeleton')).toBeInTheDocument()
  })

  test('shows the disabled note when consent is off', () => {
    renderSection({ status: 'disabled' }, { fetchEnabled: false })
    expect(screen.getByTestId('enriched-disabled')).toHaveTextContent(
      'Turn on site content fetching in Settings',
    )
  })

  test('shows an honest error note when the read failed', () => {
    renderSection({ status: 'error' })
    const note = screen.getByTestId('enriched-error')
    expect(note).toHaveTextContent('Could not load enriched content.')
    expect(note).toHaveAttribute('role', 'alert')
  })

  test('shows the empty note when nothing has been fetched', () => {
    renderSection({ status: 'empty' })
    expect(screen.getByTestId('enriched-empty')).toHaveTextContent(
      'No enriched content for this page yet.',
    )
  })

  test('renders a ready github body with source, fetched-at, description, and topic chips', () => {
    renderSection({
      status: 'ready',
      view: view({
        title: 'owner/repo',
        description: 'An async runtime',
        summary: 'An async runtime',
        topics: ['rust', 'async'],
      }),
    })

    const body = screen.getByTestId('enriched-body')
    expect(within(body).getByText('GitHub')).toBeVisible()
    expect(within(body).getByText('Fetched 2026-06-21T00:00:00Z')).toBeVisible()
    expect(within(body).getByText('owner/repo')).toBeVisible()
    expect(within(body).getByText('An async runtime')).toBeVisible()
    const topics = screen.getByTestId('enriched-topics')
    expect(within(topics).getByText('rust')).toBeVisible()
    expect(within(topics).getByText('async')).toBeVisible()
  })

  test('shows the summary separately only when it differs from the description', () => {
    renderSection({
      status: 'ready',
      view: view({
        description: 'Short desc',
        summary: 'A longer readable summary that differs',
      }),
    })
    expect(
      screen.getByText('A longer readable summary that differs'),
    ).toBeVisible()
  })

  test('a ready-but-failed status shows the honest status message, not an empty body', () => {
    renderSection({
      status: 'ready',
      view: view({ ok: false, statusKind: 'login' }),
    })
    expect(screen.getByTestId('enriched-status')).toHaveTextContent(
      'This page needs a sign-in',
    )
  })

  test('maps each non-success status to its message', () => {
    const cases: Array<[EnrichmentView['statusKind'], string]> = [
      ['empty', 'Nothing readable was found'],
      ['blocked', 'blocked in your settings'],
      ['unsupported', 'not a fetchable type'],
      ['rate-limited', 'rate-limiting requests'],
      ['error', 'The fetch failed'],
    ]
    for (const [statusKind, text] of cases) {
      const { unmount } = render(
        <PaperEnrichedContent
          state={{ status: 'ready', view: view({ ok: false, statusKind }) }}
          copy={copy}
          fetchEnabled
          fetchPending={false}
          fetchError={false}
          onFetchNow={vi.fn()}
          testId="enriched-case"
        />,
      )
      expect(screen.getByTestId('enriched-case-status')).toHaveTextContent(text)
      unmount()
    }
  })

  test('uses the generic / unknown source labels by kind', () => {
    const { rerender } = render(
      <PaperEnrichedContent
        state={{ status: 'ready', view: view({ sourceKind: 'generic' }) }}
        copy={copy}
        fetchEnabled
        fetchPending={false}
        fetchError={false}
        onFetchNow={vi.fn()}
        testId="src"
      />,
    )
    expect(
      within(screen.getByTestId('src-body')).getByText('Page summary'),
    ).toBeVisible()

    rerender(
      <PaperEnrichedContent
        state={{ status: 'ready', view: view({ sourceKind: 'unknown' }) }}
        copy={copy}
        fetchEnabled
        fetchPending={false}
        fetchError={false}
        onFetchNow={vi.fn()}
        testId="src"
      />,
    )
    expect(
      within(screen.getByTestId('src-body')).getByText('Source'),
    ).toBeVisible()
  })

  test('Fetch-now fires the handler when consent is on', () => {
    const { onFetchNow } = renderSection({ status: 'empty' })
    fireEvent.click(screen.getByTestId('enriched-fetch-now'))
    expect(onFetchNow).toHaveBeenCalledTimes(1)
  })

  test('Fetch-now is disabled with an explanatory hint when consent is off', () => {
    const { onFetchNow } = renderSection(
      { status: 'disabled' },
      { fetchEnabled: false },
    )
    const button = screen.getByTestId('enriched-fetch-now')
    expect(button).toBeDisabled()
    fireEvent.click(button)
    expect(onFetchNow).not.toHaveBeenCalled()
    expect(screen.getByTestId('enriched-fetch-disabled-hint')).toBeVisible()
  })

  test('shows a queued status and disables the button while a fetch is pending', () => {
    renderSection({ status: 'empty' }, { fetchPending: true })
    expect(screen.getByTestId('enriched-fetch-now')).toBeDisabled()
    const queued = screen.getByTestId('enriched-fetch-queued')
    expect(queued).toHaveTextContent('Queued.')
    expect(queued).toHaveAttribute('aria-live', 'polite')
  })

  test('surfaces a fetch error honestly when the last enqueue failed', () => {
    renderSection({ status: 'empty' }, { fetchError: true })
    const error = screen.getByTestId('enriched-fetch-error')
    expect(error).toHaveTextContent('Could not start the fetch')
    expect(error).toHaveAttribute('role', 'alert')
  })

  // Without a `testId` prop the component must still render every state — the
  // data-testid fallbacks resolve to `undefined`. These cover the no-testId
  // branches by asserting on visible copy instead of test ids.
  describe('without a testId prop', () => {
    function renderNoTestId(
      state: PaperEnrichedContentState,
      extra: {
        fetchEnabled?: boolean
        fetchPending?: boolean
        fetchError?: boolean
      } = {},
    ) {
      render(
        <PaperEnrichedContent
          state={state}
          copy={copy}
          fetchEnabled={extra.fetchEnabled ?? true}
          fetchPending={extra.fetchPending ?? false}
          fetchError={extra.fetchError ?? false}
          onFetchNow={vi.fn()}
        />,
      )
    }

    test('renders the loading skeleton', () => {
      renderNoTestId({ status: 'loading' })
      expect(screen.getByText('Loading enriched content…')).toBeInTheDocument()
    })

    test('renders the disabled note + disabled fetch hint', () => {
      renderNoTestId({ status: 'disabled' }, { fetchEnabled: false })
      expect(
        screen.getAllByText(/Turn on site content/i).length,
      ).toBeGreaterThan(0)
    })

    test('renders the error note', () => {
      renderNoTestId({ status: 'error' })
      expect(screen.getByRole('alert')).toHaveTextContent(
        'Could not load enriched content.',
      )
    })

    test('renders the empty note', () => {
      renderNoTestId({ status: 'empty' })
      expect(
        screen.getByText('No enriched content for this page yet.'),
      ).toBeInTheDocument()
    })

    test('renders a ready body with topics and a queued status', () => {
      renderNoTestId(
        {
          status: 'ready',
          view: view({ description: 'desc', topics: ['rust'] }),
        },
        { fetchPending: true },
      )
      expect(screen.getByText('desc')).toBeInTheDocument()
      expect(screen.getByText('rust')).toBeInTheDocument()
      expect(
        screen.getByText('Queued. This page will be enriched shortly.'),
      ).toBeInTheDocument()
    })

    test('renders a ready-but-failed status and a fetch error', () => {
      renderNoTestId(
        { status: 'ready', view: view({ ok: false, statusKind: 'blocked' }) },
        { fetchError: true },
      )
      expect(
        screen.getByText('This site is blocked in your settings.'),
      ).toBeInTheDocument()
      expect(screen.getByRole('alert')).toHaveTextContent(
        'Could not start the fetch',
      )
    })
  })
})
