/**
 * @file routes.test.ts
 * @description Pure coverage for Core Intelligence shared route grammar helpers.
 * @module lib/core-intelligence
 *
 * ## Responsibilities
 * - Verify date validation, focus parsing, labels, and hrefs for every shared insight entity reference.
 * - Keep route grammar covered without mounting promoted entity pages.
 *
 * ## Not responsible for
 * - Re-testing React Router route components.
 * - Re-testing backend entity detail payloads.
 *
 * ## Dependencies
 * - Depends only on pure route helper functions and public entity-reference types.
 *
 * ## Performance notes
 * - Pure string assertions keep this contract cheap for strict coverage and mutation gates.
 */

import { describe, expect, test } from 'vitest'
import type { InsightEntityReference } from './types'
import {
  insightEntityReferenceHref,
  insightEntityReferenceLabel,
  isLocalDateKey,
  parseInsightRouteFocus,
} from './routes'

const dateRange = { start: '2026-04-01', end: '2026-04-30' }

const t = (key: string) => key

describe('core intelligence routes', () => {
  test('validates local date keys and parses focus params defensively', () => {
    expect(isLocalDateKey('2026-04-25')).toBe(true)
    expect(isLocalDateKey('2026-4-25')).toBe(false)
    expect(isLocalDateKey('2026-02-31')).toBe(false)
    expect(
      parseInsightRouteFocus(new URLSearchParams('focusId=cmp-1')),
    ).toBeNull()
    expect(
      parseInsightRouteFocus(
        new URLSearchParams('focusType=path-flow&focusId=flow-1'),
      ),
    ).toEqual({ focusType: 'path-flow', focusId: 'flow-1' })
  })

  test('builds labels and hrefs for all shared entity references', () => {
    const context = {
      dateRange,
      preset: 'custom' as const,
      profileId: 'chrome:Default',
      focus: { focusType: 'compare-set' as const, focusId: 'cmp-focus' },
    }
    const references: Array<{
      label: string
      reference: InsightEntityReference
      href: string
    }> = [
      {
        label: '2026-04-25',
        reference: { kind: 'day', date: '2026-04-25' },
        href: '/intelligence/day/2026-04-25?profileId=chrome%3ADefault&focusType=compare-set&focusId=cmp-focus',
      },
      {
        label: 'docs.example',
        reference: { kind: 'domain', domain: 'docs.example' },
        href: '/intelligence/domain/docs.example?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome%3ADefault&focusType=compare-set&focusId=cmp-focus',
      },
      {
        label: 'queryFamilyRouteTitle',
        reference: { kind: 'queryFamily', familyId: 'family 1' },
        href: '/intelligence/query-family/family%201?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome%3ADefault&focusType=compare-set&focusId=cmp-focus',
      },
      {
        label: 'refindRouteTitle',
        reference: {
          kind: 'refindPage',
          canonicalUrl: 'https://example.com/reference',
        },
        href: '/intelligence/refind/https%3A%2F%2Fexample.com%2Freference?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome%3ADefault&focusType=compare-set&focusId=cmp-focus',
      },
      {
        label: 'sessionRouteTitle',
        reference: { kind: 'session', sessionId: 'session-1' },
        href: '/intelligence/session/session-1?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome%3ADefault&focusType=compare-set&focusId=cmp-focus',
      },
      {
        label: 'trailRouteTitle',
        reference: { kind: 'trail', trailId: 'trail-1' },
        href: '/intelligence/trail/trail-1?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome%3ADefault&focusType=compare-set&focusId=cmp-focus',
      },
      {
        label: 'compareSetRouteTitle',
        reference: { kind: 'compareSet', compareSetId: 'cmp-1' },
        href: '/intelligence/compare-set/cmp-1?range=custom&start=2026-04-01&end=2026-04-30&profileId=chrome%3ADefault&focusType=compare-set&focusId=cmp-focus',
      },
    ]

    for (const { href, label, reference } of references) {
      expect(insightEntityReferenceLabel(reference, t)).toBe(label)
      expect(insightEntityReferenceHref(reference, context)).toBe(href)
    }

    expect(
      insightEntityReferenceHref(
        { kind: 'day', date: '2026-04-25' },
        { dateRange },
      ),
    ).toBe('/intelligence/day/2026-04-25')
  })
})
