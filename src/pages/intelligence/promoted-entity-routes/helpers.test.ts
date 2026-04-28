/**
 * @file helpers.test.ts
 * @description Pure helper coverage for promoted Intelligence entity routes.
 * @module pages/intelligence/promoted-entity-routes
 *
 * ## Responsibilities
 * - Verify refind factor normalization keeps malformed backend rows harmless.
 * - Keep route tests focused on navigation while this file owns pure data guards.
 *
 * ## Not responsible for
 * - Re-testing the promoted route React pages.
 * - Re-testing Core Intelligence API readers.
 *
 * ## Dependencies
 * - Pure TypeScript helper tests only.
 *
 * ## Performance notes
 * - No DOM or async work.
 */

import { describe, expect, test } from 'vitest'
import { buildScopeCalloutCopy, normalizeRefindFactors } from './helpers'

describe('promoted entity route helpers', () => {
  test('builds archive-wide and scoped callout copy with profile id fallbacks', () => {
    const t = (key: string, vars?: Record<string, string | number>) =>
      vars ? `${key}:${vars.profile}` : key

    expect(
      buildScopeCalloutCopy({
        archiveWideBadge: 'All archive',
        archiveWideBody: 'All archive body',
        effectiveProfileId: null,
        profileScopeLabel: null,
        t,
      }),
    ).toMatchObject({
      scopeLabel: 'All archive',
      renderScopeCallout: expect.any(Function),
    })
    expect(
      buildScopeCalloutCopy({
        archiveWideBadge: 'All archive',
        archiveWideBody: 'All archive body',
        effectiveProfileId: null,
        profileScopeLabel: null,
        t,
      }).renderScopeCallout(),
    ).toEqual({
      body: 'All archive body',
      title: 'All archive',
    })
    expect(
      buildScopeCalloutCopy({
        archiveWideBadge: 'All archive',
        archiveWideBody: 'All archive body',
        effectiveProfileId: 'chrome:Default',
        profileScopeLabel: null,
        t,
      }).renderScopeCallout(),
    ).toEqual({
      body: 'scopedViewBody:chrome:Default',
      title: 'scopedViewTitle',
    })
    expect(
      buildScopeCalloutCopy({
        archiveWideBadge: 'All archive',
        archiveWideBody: 'All archive body',
        effectiveProfileId: 'chrome:Default',
        profileScopeLabel: 'Default',
        t,
      }).scopeLabel,
    ).toBe('Default')
  })

  test('drops non-object refind factors and normalizes malformed numeric fields', () => {
    expect(normalizeRefindFactors(null)).toEqual([])
    expect(
      normalizeRefindFactors([
        null,
        'bad',
        {
          signal: 'typed-revisit',
          rawValue: Number.POSITIVE_INFINITY,
          weight: 0.4,
          contribution: Number.NaN,
        },
        {
          signal: null,
          rawValue: 3,
          weight: Number.NEGATIVE_INFINITY,
          contribution: 2,
        },
      ]),
    ).toEqual([
      {
        signal: 'typed-revisit',
        rawValue: 0,
        weight: 0.4,
        contribution: 0,
      },
      {
        signal: '',
        rawValue: 3,
        weight: 0,
        contribution: 2,
      },
    ])
  })
})
