import { describe, expect, test, vi } from 'vitest'
import {
  buildAnalyticsPayload,
  shouldSendAnalytics,
  trackAnalyticsEvent,
} from './analytics'

describe('analytics helpers', () => {
  test('requires explicit opt-in, desktop production, endpoint, and fetch', () => {
    expect(
      shouldSendAnalytics(
        {
          enabled: true,
          consentGrantedAt: '2026-04-10T00:00:00Z',
        },
        {
          endpoint: 'https://analytics.example.test/events',
          isDesktop: true,
          isProduction: true,
          fetchImpl: fetch,
        },
      ),
    ).toBe(true)

    expect(
      shouldSendAnalytics(
        {
          enabled: false,
          consentGrantedAt: null,
        },
        {
          endpoint: 'https://analytics.example.test/events',
          isDesktop: true,
          isProduction: true,
          fetchImpl: fetch,
        },
      ),
    ).toBe(false)
  })

  test('builds coarse payloads without archive content fields', () => {
    const payload = buildAnalyticsPayload(
      {
        type: 'update-lifecycle',
        screen: 'settings',
        action: 'download-and-install',
        status: 'installed',
        version: '0.2.0',
      },
      { version: '0.1.0' },
      '2026-04-10T00:00:00Z',
    )

    expect(payload).toEqual({
      type: 'update-lifecycle',
      occurredAt: '2026-04-10T00:00:00Z',
      appVersion: '0.1.0',
      screen: 'settings',
      action: 'download-and-install',
      status: 'installed',
      version: '0.2.0',
    })
    expect(payload).not.toHaveProperty('url')
    expect(payload).not.toHaveProperty('query')
    expect(payload).not.toHaveProperty('profileId')
    expect(payload).not.toHaveProperty('archivePath')
    expect(payload).not.toHaveProperty('runId')
    expect(payload).not.toHaveProperty('prompt')
  })

  test('posts only when the runtime boundary is satisfied', async () => {
    const fetchSpy = vi.fn().mockResolvedValue({ ok: true })

    const sent = await trackAnalyticsEvent(
      {
        enabled: true,
        consentGrantedAt: '2026-04-10T00:00:00Z',
      },
      {
        type: 'route-view',
        route: '/settings',
        screen: 'settings',
        language: 'en',
      },
      { version: '0.1.0' },
      {
        endpoint: 'https://analytics.example.test/events',
        isDesktop: true,
        isProduction: true,
        fetchImpl: fetchSpy as typeof fetch,
        now: () => '2026-04-10T00:00:00Z',
      },
    )

    expect(sent).toBe(true)
    expect(fetchSpy).toHaveBeenCalledWith(
      'https://analytics.example.test/events',
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({
          type: 'route-view',
          occurredAt: '2026-04-10T00:00:00Z',
          appVersion: '0.1.0',
          route: '/settings',
          screen: 'settings',
          language: 'en',
        }),
      }),
    )
  })
})
