/**
 * @file local-host-fixtures.ts
 * @description Trusted local-host fixture builders shared by Intelligence surface suites.
 * @module pages/intelligence-surfaces
 *
 * ## Responsibilities
 * - Build stable local-host preview payloads for Settings and external-output tests.
 * - Keep the trusted-only bundle grammar in one place so split suites do not drift.
 *
 * ## Non-Responsibilities
 * - Does not render UI or reset global test state.
 * - Does not own generic section-envelope helpers or route rendering.
 *
 * ## Dependencies
 * - Depends on typed Core Intelligence local-host payload contracts.
 *
 * ## Performance Notes
 * - Returns static fixture data only; no I/O or archive seeding happens here.
 */

import type {
  IntelligenceLocalHostBundle,
  IntelligenceLocalHostPreview,
} from '../../lib/core-intelligence/types'

/**
 * Produces the canonical trusted local-host preview fixture used by the split
 * Settings external-output suites.
 *
 * @param locale The locale the trusted host preview should report.
 * @param profileId Optional profile scope recorded in the generated bundle.
 * @returns A deterministic local-host preview payload with trusted-only cards.
 */
export function createLocalHostPreview(
  locale: string,
  profileId: string | null = 'chrome:Default',
): IntelligenceLocalHostPreview {
  const bundle: IntelligenceLocalHostBundle = {
    bundleVersion: 'pathkeep.core-intelligence.local-host.v1',
    hostId: 'browser-snippet-v1',
    generatedAt: '2026-04-18T10:15:00Z',
    locale,
    dateRange: { start: '2026-03-17', end: '2026-04-17' },
    profileId,
    embedCards: [
      {
        cardId: 'digest:visits',
        cardType: 'digest',
        title: 'Visits',
        eyebrow: '2026-03-17 → 2026-04-17',
        body: 'Preview fixture for the trusted local snippet host.',
        metricLabel: 'visit_count',
        metricValue: '128',
        href: null,
        internalOnly: false,
      },
      {
        cardId: 'refind:sqlite',
        cardType: 'refind_page',
        title: 'SQLite WAL guide',
        eyebrow: 'Refind',
        body: 'This page kept resurfacing across 4 days and 3 trails.',
        metricLabel: 'refind_score',
        metricValue: '0.82',
        href: 'https://sqlite.org/wal.html',
        internalOnly: true,
      },
    ],
    widgetSnapshot: {
      generatedAt: '2026-04-18T10:15:00Z',
      dateRange: { start: '2026-03-17', end: '2026-04-17' },
      digestSummary: {
        dateRange: { start: '2026-03-17', end: '2026-04-17' },
        totalVisits: {
          value: 128,
          trend: 'up',
          previousValue: 120,
          changePercent: 7,
        },
        totalSearches: {
          value: 32,
          trend: 'up',
          previousValue: 28,
          changePercent: 14,
        },
        newDomains: {
          value: 9,
          trend: 'up',
          previousValue: 8,
          changePercent: 13,
        },
        deepReadPages: {
          value: 5,
          trend: 'up',
          previousValue: 4,
          changePercent: 25,
        },
        refindPages: {
          value: 3,
          trend: 'up',
          previousValue: 2,
          changePercent: 50,
        },
      },
      highlights: [
        {
          cardId: 'refind:sqlite',
          cardType: 'refind_page',
          title: 'SQLite WAL guide',
          eyebrow: 'Refind',
          body: 'This page kept resurfacing across 4 days and 3 trails.',
          metricLabel: 'refind_score',
          metricValue: '0.82',
          href: 'https://sqlite.org/wal.html',
          internalOnly: true,
        },
      ],
      notes: [
        'Widget snapshots only expose aggregate Core Intelligence read models.',
      ],
    },
    publicSnapshot: {
      generatedAt: '2026-04-18T10:15:00Z',
      dateRange: { start: '2026-03-17', end: '2026-04-17' },
      digestSummary: {
        dateRange: { start: '2026-03-17', end: '2026-04-17' },
        totalVisits: {
          value: 128,
          trend: 'up',
          previousValue: 120,
          changePercent: 7,
        },
        totalSearches: {
          value: 32,
          trend: 'up',
          previousValue: 28,
          changePercent: 14,
        },
        newDomains: {
          value: 9,
          trend: 'up',
          previousValue: 8,
          changePercent: 13,
        },
        deepReadPages: {
          value: 5,
          trend: 'up',
          previousValue: 4,
          changePercent: 25,
        },
        refindPages: {
          value: 3,
          trend: 'up',
          previousValue: 2,
          changePercent: 50,
        },
      },
      topDomains: ['sqlite.org', 'github.com'],
      searchEngines: [
        { searchEngine: 'google', displayName: 'Google', searchCount: 18 },
      ],
      discoveryTrend: {
        availableYears: [],
        points: [
          {
            dateKey: '2026-04-07',
            discoveryRate: 0.35,
            newDomainCount: 4,
            totalVisits: 22,
          },
        ],
      },
      notes: [
        'Public snapshots intentionally omit visit-level identifiers and direct page URLs.',
      ],
    },
    trustedOnlyCardIds: ['refind:sqlite'],
    trustedOnlyCardCount: 1,
    boundaryNotes: [
      'This local host only uses deterministic Core Intelligence read models.',
      'Trusted-only cards must stay inside PathKeep-controlled local surfaces.',
    ],
  }

  return {
    artifactRoot:
      '/Users/tim/Library/Application Support/PathKeep/integrations/core-intelligence/browser-snippet-v1',
    entryFilePath:
      '/Users/tim/Library/Application Support/PathKeep/integrations/core-intelligence/browser-snippet-v1/index.html',
    generatedFiles: [
      {
        relativePath:
          'integrations/core-intelligence/browser-snippet-v1/index.html',
        absolutePath:
          '/Users/tim/Library/Application Support/PathKeep/integrations/core-intelligence/browser-snippet-v1/index.html',
        purpose:
          'Core Intelligence snippet that can be opened directly in a local browser.',
        contents:
          '<!doctype html><title>PathKeep Core Intelligence Snippet</title>',
      },
      {
        relativePath:
          'integrations/core-intelligence/browser-snippet-v1/bundle.json',
        absolutePath:
          '/Users/tim/Library/Application Support/PathKeep/integrations/core-intelligence/browser-snippet-v1/bundle.json',
        purpose:
          'Machine-readable JSON bundle for the same local host artifact.',
        contents: JSON.stringify(bundle, null, 2),
      },
    ],
    bundle,
    boundaryNotes: bundle.boundaryNotes,
    manualSteps: [
      'Review index.html and bundle.json before handing this folder to another trusted local tool.',
      'Open index.html from this folder inside a trusted local browser surface.',
    ],
    warnings: [
      'This local snippet includes trusted-only cards and should not be treated like a public export.',
    ],
    installedHost: null,
  }
}
