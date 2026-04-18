/**
 * Verifies the typed Core Intelligence IPC wrappers.
 *
 * Why this file exists:
 * - The backend already ships payload-provider commands, so the front-end draft contract should
 *   prove it sends the exact command names and request envelopes we intend to support.
 * - These wrappers are tiny, but getting the invoke shape wrong would silently break future host
 *   consumers and be hard to spot from route tests alone.
 */

import { beforeEach, describe, expect, test, vi } from 'vitest'

const { callMock } = vi.hoisted(() => ({
  callMock: vi.fn(),
}))

vi.mock('../backend-client/shared', () => ({
  call: callMock,
}))

describe('core intelligence api', () => {
  beforeEach(() => {
    callMock.mockReset()
    callMock.mockResolvedValue({})
  })

  test('requests embed cards through the backend payload-provider command', async () => {
    const { getIntelligenceEmbedCards } = await import('./api')

    await getIntelligenceEmbedCards(
      { start: '2024-04-01', end: '2024-04-30' },
      'chrome:Default',
      6,
    )

    expect(callMock).toHaveBeenCalledWith('get_intelligence_embed_cards', {
      request: {
        dateRange: { start: '2024-04-01', end: '2024-04-30' },
        profileId: 'chrome:Default',
        limit: 6,
      },
    })
  })

  test('requests widget snapshots through the backend payload-provider command', async () => {
    const { getIntelligenceWidgetSnapshot } = await import('./api')

    await getIntelligenceWidgetSnapshot(
      { start: '2024-04-01', end: '2024-04-30' },
      'chrome:Default',
      4,
    )

    expect(callMock).toHaveBeenCalledWith(
      'get_intelligence_widget_snapshot',
      {
        request: {
          dateRange: { start: '2024-04-01', end: '2024-04-30' },
          profileId: 'chrome:Default',
          limit: 4,
        },
      },
    )
  })

  test('requests public snapshots through the backend payload-provider command', async () => {
    const { getIntelligencePublicSnapshot } = await import('./api')

    await getIntelligencePublicSnapshot(
      { start: '2024-04-01', end: '2024-04-30' },
      'chrome:Default',
    )

    expect(callMock).toHaveBeenCalledWith(
      'get_intelligence_public_snapshot',
      {
        request: {
          dateRange: { start: '2024-04-01', end: '2024-04-30' },
          profileId: 'chrome:Default',
        },
      },
    )
  })
})
