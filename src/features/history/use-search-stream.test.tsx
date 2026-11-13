/**
 * Guards search ownership across React's commit and effect-cleanup phases.
 * Responsibilities: reproduce a queued old frame arriving as a search is cleared,
 * and preserve valid placeholders and events that precede the start response.
 * Not responsible for: backend ranking or transport decoding, covered by E2E.
 *
 * Failure modes written before the fix:
 * - an old frame runs after the null-request render reset but before passive
 *   cleanup, bringing cleared rows back as the next search's placeholders;
 * - rejecting old frames also drops a current search's early result;
 * - replacing a search discards its settled rows before the new result arrives.
 * This scheduling race needs a deterministic React test. Only the transport and
 * animation-frame scheduler are controlled; the hook and React renderer are real.
 */
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, useLayoutEffect } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import type {
  IndexerState,
  SearchEvent,
  SearchRequest,
  SearchStep,
} from '@/lib/backend-client/search'
import type { SearchStream } from '@/lib/ipc/search-stream'
import { searchRevisionKey, useSearchStream } from './use-search-stream'

const transport = vi.hoisted(() => ({
  start:
    vi.fn<
      (
        request: SearchRequest,
        listener: (event: SearchEvent) => void,
      ) => Promise<SearchStream>
    >(),
}))

vi.mock('@/lib/backend-client/search', () => ({
  searchClient: { start: transport.start },
}))

Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })

const ready: IndexerState = {
  phase: 'ready',
  documentsProcessed: 1,
  documentsTotal: 1,
  appliedSeq: 1,
  targetSeq: 1,
  error: null,
}

function result(query: string): SearchStep {
  const progress = {
    documentsProcessed: 1,
    segmentsCompleted: 1,
    segmentsTotal: 1,
    statisticsFieldsProcessed: 1,
    statisticsFieldsTotal: 1,
  }
  return {
    page: 1,
    items: [
      {
        pageId: 1,
        score: 1,
        visit: {
          id: 1,
          profileId: 'chrome:Default',
          url: `https://example.test/${query}`,
          domain: 'example.test',
          visitedAt: '2026-01-01T00:00:00.000Z',
          visitTime: Date.UTC(2026, 0, 1),
          sourceVisitId: 1,
        },
      },
    ],
    provisional: false,
    progress,
    count: { value: 1, exact: true },
    visits: { value: 1, exact: true },
    countProgress: progress,
    cursor: null,
    indexer: ready,
    snapshotSeq: 1,
    coverageComplete: true,
  }
}

const listeners: ((event: SearchEvent) => void)[] = []
const unmounts: (() => void)[] = []
let pendingFrame: FrameRequestCallback

beforeEach(() => {
  listeners.length = 0
  transport.start.mockReset()
  transport.start.mockImplementation((request, listener) => {
    listeners.push(listener)
    return Promise.resolve({
      sessionId: request.query ?? 'empty',
      indexer: ready,
      close: vi.fn(),
    })
  })
  let frameId = 0
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    pendingFrame = callback
    return ++frameId
  })
  vi.stubGlobal('cancelAnimationFrame', vi.fn())
})

afterEach(() => {
  for (const unmount of unmounts.splice(0)) unmount()
  vi.unstubAllGlobals()
})

function mount() {
  const client = new QueryClient()
  client.setQueryData(searchRevisionKey, 0)
  const root = createRoot(document.createElement('div'))
  let latest: ReturnType<typeof useSearchStream>
  function Harness({
    request,
    onCommit,
  }: {
    request: SearchRequest | null
    onCommit?: () => void
  }) {
    latest = useSearchStream(request)
    useLayoutEffect(() => onCommit?.(), [onCommit])
    return null
  }
  unmounts.push(() => {
    act(() => root.unmount())
    client.clear()
  })
  return {
    read: () => latest,
    render: async (request: SearchRequest | null, onCommit?: () => void) => {
      await act(async () => {
        root.render(
          <QueryClientProvider client={client}>
            <Harness request={request} onCommit={onCommit} />
          </QueryClientProvider>,
        )
        // Let the subscription's start response settle inside this act scope.
        await Promise.resolve()
      })
    },
  }
}

it('clearing a search rejects its queued frame before it can become a placeholder', async () => {
  const view = mount()
  await view.render({ query: 'first' })
  act(() => listeners[0]({ type: 'step', data: result('first') }))
  const oldFrame = pendingFrame

  // Run the frame in the commit gap, before a passive subscription can clean up.
  await view.render(null, () => oldFrame(0))
  expect(view.read().placeholder).toBeNull()
  expect(view.read().current?.pages).toEqual([])

  await view.render({ query: 'second' })
  expect(view.read().placeholder).toBeNull()
  act(() => listeners.at(-1)!({ type: 'step', data: result('second') }))
  act(() => pendingFrame(0))
  expect(view.read().current?.pages[0].items[0].visit.url).toBe(
    'https://example.test/second',
  )
})

it('keeps settled rows until the replacement search delivers its first step', async () => {
  const view = mount()
  await view.render({ query: 'first' })
  act(() => listeners[0]({ type: 'step', data: result('first') }))
  act(() => pendingFrame(0))
  await view.render({ query: 'second' })
  expect(view.read().placeholder?.pages[0].items[0].visit.url).toBe(
    'https://example.test/first',
  )
  act(() => listeners[1]({ type: 'step', data: result('second') }))
  act(() => pendingFrame(0))
  expect(view.read().placeholder).toBeNull()
  expect(view.read().current?.pages[0].items[0].visit.url).toBe(
    'https://example.test/second',
  )
})

it('accepts a live step while the start response is still pending', async () => {
  transport.start.mockImplementation((_request, listener) => {
    listeners.push(listener)
    return new Promise(() => {})
  })
  const view = mount()
  await view.render({ query: 'early' })
  act(() => listeners[0]({ type: 'step', data: result('early') }))
  act(() => pendingFrame(0))
  expect(view.read().current?.pages[0].items[0].visit.url).toBe(
    'https://example.test/early',
  )
})
