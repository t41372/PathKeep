/**
 * @file external-outputs-shared.test.ts
 * @description Protects the pure helper contract behind the split Settings external-output review surface.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Verify that review JSON formatting stays deterministic.
 * - Verify that digest metric rows preserve locale formatting and shipped label order.
 *
 * ## Not responsible for
 * - Rendering the Settings page or tab components
 * - Verifying async Core Intelligence fetch behavior
 *
 * ## Dependencies
 * - Depends only on the pure helpers in `external-outputs-shared.ts`.
 *
 * ## Performance notes
 * - Keeps the split helper module testable without booting a route harness.
 */

import { describe, expect, test } from 'vitest'
import { buildDigestMetricItems, prettyJson } from './external-outputs-shared'

describe('external output shared helpers', () => {
  test('prettyJson keeps review payloads stable and indented', () => {
    expect(prettyJson({ a: 1, nested: { ok: true } })).toBe(
      '{\n  "a": 1,\n  "nested": {\n    "ok": true\n  }\n}',
    )
  })

  test('buildDigestMetricItems preserves shipped order and locale formatting', () => {
    const items = buildDigestMetricItems(
      {
        dateRange: { start: '2026-03-17', end: '2026-04-17' },
        totalVisits: {
          value: 1200,
          previousValue: 1000,
          changePercent: 20,
          trend: 'up',
        },
        totalSearches: {
          value: 250,
          previousValue: 200,
          changePercent: 25,
          trend: 'up',
        },
        newDomains: {
          value: 30,
          previousValue: 20,
          changePercent: 50,
          trend: 'up',
        },
        deepReadPages: {
          value: 11,
          previousValue: 10,
          changePercent: 10,
          trend: 'up',
        },
        refindPages: {
          value: 5,
          previousValue: 4,
          changePercent: 25,
          trend: 'up',
        },
      },
      'en-US',
      (key) => `label:${key}`,
    )

    expect(items).toEqual([
      { label: 'label:digestVisits', value: '1,200' },
      { label: 'label:digestSearches', value: '250' },
      { label: 'label:digestNewSites', value: '30' },
      { label: 'label:digestDeepRead', value: '11' },
      { label: 'label:digestRefind', value: '5' },
    ])
  })
})
