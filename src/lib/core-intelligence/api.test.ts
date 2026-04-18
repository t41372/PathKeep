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

    expect(callMock).toHaveBeenCalledWith('get_intelligence_widget_snapshot', {
      request: {
        dateRange: { start: '2024-04-01', end: '2024-04-30' },
        profileId: 'chrome:Default',
        limit: 4,
      },
    })
  })

  test('requests public snapshots through the backend payload-provider command', async () => {
    const { getIntelligencePublicSnapshot } = await import('./api')

    await getIntelligencePublicSnapshot(
      { start: '2024-04-01', end: '2024-04-30' },
      'chrome:Default',
    )

    expect(callMock).toHaveBeenCalledWith('get_intelligence_public_snapshot', {
      request: {
        dateRange: { start: '2024-04-01', end: '2024-04-30' },
        profileId: 'chrome:Default',
      },
    })
  })

  test('requests local host previews through the backend host-preview command', async () => {
    const { previewIntelligenceLocalHost } = await import('./api')

    await previewIntelligenceLocalHost(
      { start: '2024-04-01', end: '2024-04-30' },
      'zh-CN',
      'chrome:Default',
    )

    expect(callMock).toHaveBeenCalledWith('preview_intelligence_local_host', {
      request: {
        dateRange: { start: '2024-04-01', end: '2024-04-30' },
        profileId: 'chrome:Default',
        locale: 'zh-CN',
      },
    })
  })

  test('requests local host builds through the backend host-build command', async () => {
    const { buildIntelligenceLocalHost } = await import('./api')

    await buildIntelligenceLocalHost(
      { start: '2024-04-01', end: '2024-04-30' },
      'en',
      'chrome:Default',
    )

    expect(callMock).toHaveBeenCalledWith('build_intelligence_local_host', {
      request: {
        dateRange: { start: '2024-04-01', end: '2024-04-30' },
        profileId: 'chrome:Default',
        locale: 'en',
      },
    })
  })
})
