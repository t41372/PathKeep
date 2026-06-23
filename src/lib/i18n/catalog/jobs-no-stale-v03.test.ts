/**
 * @file jobs-no-stale-v03.test.ts
 * @description Guards against stale "v0.3 / tracked for" AI copy regressing onto shipped surfaces.
 * @module lib/i18n/catalog
 *
 * Review-fix M-8: the assistant + embedding queue and the optional-AI features shipped, so any copy
 * that still says they are "tracked for v0.3" / "coming in v0.3" is internally contradictory (it
 * renders directly above their LIVE queue counts). These assertions pin the honest off-by-default
 * framing so the drift cannot return on the reachable surfaces. They scan all three shipped locales.
 */

import { describe, expect, it } from 'vitest'
import { jobsNamespaceCatalog } from './jobs'
import { dashboardNamespaceCatalog } from './dashboard'
import { intelligenceOverviewAndRoutesNamespace } from './intelligence-overview-and-routes'

const LOCALES = ['en', 'zh-CN', 'zh-TW'] as const
// Stale-version markers, English + both Chinese phrasings, that must not appear on shipped AI copy.
// Covers the removed dashboard optional-AI keys (`...ComingBadge` / `...DeferredTooltip`): the
// "v0.3 / coming / tracked-for" version framing *and* the tooltip's "coming in a future update" /
// "后续/後續版本开放/開放" deferral phrasing, so neither can regress onto a shipped surface.
const STALE_MARKERS = [
  'v0.3',
  'tracked for',
  'coming in v0.3',
  'coming in a future update',
  '排入 v0.3',
  'v0.3 开放',
  'v0.3 開放',
  '后续版本开放',
  '後續版本開放',
]

function assertNoStaleMarkers(label: string, value: string) {
  for (const marker of STALE_MARKERS) {
    expect(
      value.includes(marker),
      `${label} must not contain stale "${marker}": ${value}`,
    ).toBe(false)
  }
}

describe('M-8 — no stale v0.3 AI copy on shipped surfaces', () => {
  it('jobs queueSummaryBody is honest off-by-default copy in every locale', () => {
    for (const locale of LOCALES) {
      const body = jobsNamespaceCatalog[locale].queueSummaryBody
      assertNoStaleMarkers(`jobs.queueSummaryBody[${locale}]`, body)
      // It must be non-empty real copy (not an English placeholder leaking into the Chinese locales).
      expect(body.trim().length).toBeGreaterThan(0)
    }
    // The English copy carries the honest "available but off" framing the contentFetch copy uses.
    expect(jobsNamespaceCatalog.en.queueSummaryBody.toLowerCase()).toContain(
      'off by default',
    )
  })

  it('the reachable AI-disabled status description is honest off-by-default copy', () => {
    for (const locale of LOCALES) {
      assertNoStaleMarkers(
        `intelligence.statusDisabledDescription[${locale}]`,
        intelligenceOverviewAndRoutesNamespace[locale]
          .statusDisabledDescription,
      )
    }
  })

  it('no dashboard copy carries stale "v0.3 / coming / tracked-for" AI framing', () => {
    // The dead `optionalAiComingBadge` / `optionalAiDeferredBody` / `optionalAiDeferredTooltip`
    // keys were removed once the assistant + embeddings shipped; the dashboard namespace owns no
    // legitimate deferred-roadmap copy (contentFetch* / readableContent* live in jobs / settings),
    // so a whole-namespace scan guards against any stale AI key regressing here without false
    // positives. Scoped to every shipped value in all three locales.
    for (const locale of LOCALES) {
      const namespace = dashboardNamespaceCatalog[locale]
      for (const [key, value] of Object.entries(namespace)) {
        assertNoStaleMarkers(`dashboard.${key}[${locale}]`, value)
      }
    }
  })
})
