/**
 * Behaviour tests for the site-content-enrichment client (W-ENRICH-1).
 *
 * Why this file exists:
 * - The consent + detail surfaces depend on this client routing to the exact
 *   desktop command names with the exact camelCase arg shapes the backend
 *   payloads decode. A drift here would surface as a silent no-op on a
 *   privacy-sensitive surface.
 * - It also pins the browser-preview degradation: the Vercel preview has no
 *   desktop backend, so read paths must return the inert "off / never fetched"
 *   default instead of throwing (mirroring how `stars.ts` degrades).
 */

import { beforeEach, describe, expect, test, vi } from 'vitest'

const {
  invokeCommandMock,
  hasDesktopCommandTransportMock,
  backendHarnessMock,
} = vi.hoisted(() => ({
  invokeCommandMock: vi.fn(),
  hasDesktopCommandTransportMock: vi.fn(() => true),
  backendHarnessMock: {
    call: vi.fn(),
  },
}))

vi.mock('../ipc/bridge', () => ({
  invokeCommand: invokeCommandMock,
}))

vi.mock('../runtime', () => ({
  hasDesktopCommandTransport: hasDesktopCommandTransportMock,
}))

vi.mock('../backend', () => ({
  backendTestHarness: backendHarnessMock,
}))

describe('content-enrichment client', () => {
  beforeEach(() => {
    invokeCommandMock.mockReset()
    backendHarnessMock.call.mockReset()
    hasDesktopCommandTransportMock.mockReturnValue(true)
    ;(
      window as Window & {
        __PATHKEEP_DESKTOP_COMMAND_METRICS__?: unknown[]
      }
    ).__PATHKEEP_DESKTOP_COMMAND_METRICS__ = []
  })

  test('reads content-fetch settings over the desktop transport', async () => {
    const settings = {
      enabled: true,
      extractors: [{ extractorId: 'github-repo', enabled: true }],
      domains: [],
      queuedJobs: 0,
      runningJobs: 0,
      failedJobs: 0,
      storedRecords: 3,
    }
    invokeCommandMock.mockResolvedValueOnce(settings)

    const { contentEnrichmentClient } = await import('./content-enrichment')
    const result = await contentEnrichmentClient.getContentFetchSettings()

    expect(invokeCommandMock).toHaveBeenCalledWith(
      'get_content_fetch_settings',
      {},
    )
    expect(result).toEqual(settings)
  })

  test('returns the inert default settings in browser-preview', async () => {
    hasDesktopCommandTransportMock.mockReturnValue(false)

    const { contentEnrichmentClient } = await import('./content-enrichment')
    const result = await contentEnrichmentClient.getContentFetchSettings()

    expect(result).toEqual({
      enabled: false,
      extractors: [],
      domains: [],
      queuedJobs: 0,
      runningJobs: 0,
      failedJobs: 0,
      storedRecords: 0,
    })
    // The unimplemented command must never reach either transport in preview.
    expect(invokeCommandMock).not.toHaveBeenCalled()
    expect(backendHarnessMock.call).not.toHaveBeenCalled()
  })

  test('persists content-fetch settings and returns the snapshot', async () => {
    const snapshot = { config: {}, aiStatus: {} }
    invokeCommandMock.mockResolvedValueOnce(snapshot)
    const settings = {
      enabled: true,
      extractors: [{ extractorId: 'generic-readable', enabled: false }],
      domains: [{ domain: 'blocked.test', allowed: false }],
      queuedJobs: 0,
      runningJobs: 0,
      failedJobs: 0,
      storedRecords: 0,
    }

    const { contentEnrichmentClient } = await import('./content-enrichment')
    const result = await contentEnrichmentClient.setContentFetchSettings(
      settings as never,
    )

    expect(invokeCommandMock).toHaveBeenCalledWith(
      'set_content_fetch_settings',
      {
        settings,
      },
    )
    expect(result).toBe(snapshot)
  })

  test('lists one visit enrichment over the desktop transport', async () => {
    const rows = [
      {
        contentSource: 'github-repo',
        fetchStatus: 'success',
        fetchedAt: '2026-06-21T00:00:00Z',
        summary: 'A repo',
        metadataJson: '{"topics":["rust"]}',
      },
    ]
    invokeCommandMock.mockResolvedValueOnce(rows)

    const { contentEnrichmentClient } = await import('./content-enrichment')
    const result = await contentEnrichmentClient.listVisitEnrichment(42)

    expect(invokeCommandMock).toHaveBeenCalledWith('list_visit_enrichment', {
      historyId: 42,
    })
    expect(result).toEqual(rows)
  })

  test('returns no enrichment rows in browser-preview', async () => {
    hasDesktopCommandTransportMock.mockReturnValue(false)

    const { contentEnrichmentClient } = await import('./content-enrichment')
    const result = await contentEnrichmentClient.listVisitEnrichment(7)

    expect(result).toEqual([])
    expect(invokeCommandMock).not.toHaveBeenCalled()
    expect(backendHarnessMock.call).not.toHaveBeenCalled()
  })

  test('triggers a manual fetch-now PME request', async () => {
    const fetchResult = { jobId: 9, state: 'queued', note: 'queued' }
    invokeCommandMock.mockResolvedValueOnce(fetchResult)
    const request = {
      historyId: 5,
      profileId: 'chrome:Default',
      url: 'https://github.com/owner/repo',
      title: 'owner/repo',
    }

    const { contentEnrichmentClient } = await import('./content-enrichment')
    const result = await contentEnrichmentClient.contentFetchNow(request)

    expect(invokeCommandMock).toHaveBeenCalledWith('content_fetch_now', {
      request,
    })
    expect(result).toEqual(fetchResult)
  })

  test('enqueues the working set with an explicit limit', async () => {
    invokeCommandMock.mockResolvedValueOnce(12)

    const { contentEnrichmentClient } = await import('./content-enrichment')
    const result =
      await contentEnrichmentClient.enqueueContentFetchWorkingSet(50)

    expect(invokeCommandMock).toHaveBeenCalledWith(
      'enqueue_content_fetch_working_set',
      { limit: 50 },
    )
    expect(result).toBe(12)
  })

  test('enqueues the working set with a null limit when omitted', async () => {
    invokeCommandMock.mockResolvedValueOnce(0)

    const { contentEnrichmentClient } = await import('./content-enrichment')
    const result = await contentEnrichmentClient.enqueueContentFetchWorkingSet()

    expect(invokeCommandMock).toHaveBeenCalledWith(
      'enqueue_content_fetch_working_set',
      { limit: null },
    )
    expect(result).toBe(0)
  })

  test('enqueues nothing in browser-preview', async () => {
    hasDesktopCommandTransportMock.mockReturnValue(false)

    const { contentEnrichmentClient } = await import('./content-enrichment')
    const result =
      await contentEnrichmentClient.enqueueContentFetchWorkingSet(99)

    expect(result).toBe(0)
    expect(invokeCommandMock).not.toHaveBeenCalled()
    expect(backendHarnessMock.call).not.toHaveBeenCalled()
  })
})
