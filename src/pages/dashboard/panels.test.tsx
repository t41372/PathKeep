/**
 * @file panels.test.tsx
 * @description Focused render coverage for Dashboard panel owners that route tests mock out.
 * @module pages/dashboard
 *
 * ## Responsibilities
 * - Verify the compact On This Day panel renders loading, entries, domain links, and fallback copy.
 * - Keep dashboard route tests free to mock panel layout without losing panel-level coverage.
 *
 * ## Not responsible for
 * - Re-testing DashboardPage data loading.
 * - Re-testing Core Intelligence day/domain route implementations.
 *
 * ## Dependencies
 * - Uses MemoryRouter because panel entries render day/domain links.
 *
 * ## Performance notes
 * - Pure render fixtures keep panel coverage cheap and bounded.
 */

import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import type { OnThisDayEntry } from '../../lib/core-intelligence/types'
import type {
  BackupRunOverview,
  BrowserProfile,
  DashboardSnapshot,
} from '../../lib/types'
import {
  DashboardArchiveBoundaryPanel,
  DashboardIntelligencePanel,
  DashboardOnThisDayPanel,
  DashboardRecentRunsPanel,
  DashboardStorageFootprintPanel,
  DashboardZeroStateChecklistPanel,
} from './panels'

const intelligenceT = (key: string, vars?: Record<string, string | number>) =>
  vars ? `${key}:${JSON.stringify(vars)}` : key
const t = (key: string, vars?: Record<string, string | number>) =>
  vars ? `${key}:${JSON.stringify(vars)}` : key

describe('Dashboard panels', () => {
  test('renders recent run fallback labels, status, source, and started time', () => {
    render(
      <MemoryRouter>
        <DashboardRecentRunsPanel
          dashboard={dashboardFixture({
            recentRuns: [
              runFixture({
                finishedAt: null,
                profileScope: undefined as unknown as string[],
                runType: null as unknown as string,
                status: 'failed',
              }),
            ],
          })}
          language="en"
          runSourceSummary={(profileScope) =>
            profileScope ? profileScope.join(', ') : 'all profiles'
          }
          t={t}
        />
      </MemoryRouter>,
    )

    expect(screen.getByRole('link', { name: '#8' })).toHaveAttribute(
      'href',
      '/audit?run=8',
    )
    expect(screen.getByText('audit.runTypeBackup')).toBeVisible()
    expect(screen.getByText('all profiles')).toBeVisible()
    expect(screen.getByLabelText('common.statusFailed')).toHaveClass(
      'status-pending',
    )
  })

  test('renders archive boundaries for detected, missing, and empty profiles', () => {
    const { rerender } = render(
      <MemoryRouter>
        <DashboardArchiveBoundaryPanel
          commonT={t}
          selectedProfiles={[
            browserProfileFixture({
              browserName: 'Safari',
              historyExists: true,
              retentionBoundary: { kind: 'macos-safari', localDays: null },
            }),
            browserProfileFixture({
              browserName: 'Google Chrome',
              historyExists: false,
              profileId: 'chrome:Default',
              profileName: 'Default',
            }),
          ]}
          t={t}
        />
      </MemoryRouter>,
    )

    expect(screen.getByText('Safari / Default')).toBeVisible()
    expect(screen.getByText('dashboard.historyDetected')).toBeVisible()
    expect(
      screen.getByText('browserRetentionSafariLabel:{"days":365}'),
    ).toBeVisible()
    expect(screen.getByText('Google Chrome / Default')).toBeVisible()
    expect(screen.getByText('dashboard.historyMissing')).toBeVisible()

    rerender(
      <MemoryRouter>
        <DashboardArchiveBoundaryPanel
          commonT={t}
          selectedProfiles={[]}
          t={t}
        />
      </MemoryRouter>,
    )

    expect(screen.getByText('dashboard.zeroStateNoBrowsers')).toBeVisible()
  })

  test('renders zero-width storage and empty AI provider fallbacks', () => {
    render(
      <MemoryRouter>
        <DashboardStorageFootprintPanel
          language="en"
          storageSegments={[
            {
              detail: 'No storage yet',
              label: 'Archive',
              tone: 'accent',
              value: 0,
            },
          ]}
          totalStorage={0}
          t={t}
        />
        <DashboardIntelligencePanel
          aiMeta={{ description: 'AI disabled', label: 'offline' } as never}
          backgroundQueueCount={null}
          embeddingProviderId={null}
          language="en"
          llmProviderId={null}
          t={t}
        />
      </MemoryRouter>,
    )

    expect(
      screen.getByText('dashboard.storageTotal:{"size":"0 B"}'),
    ).toBeVisible()
    expect(document.querySelector('.storage-bar > .accent')).toHaveStyle({
      width: '0%',
    })
    expect(screen.getByText('settings.disabled')).toBeVisible()
    expect(screen.getByText('dashboard.embeddingFallback')).toBeVisible()
    expect(screen.getByText('—')).toBeVisible()
  })

  test('renders dashboard AI quick links when optional AI is release-enabled', async () => {
    vi.resetModules()
    vi.doMock('../../lib/release-capabilities', () => ({
      deferredFeatureReleaseLabel: 'v0.2',
      optionalAiFeaturesAvailable: true,
      readableContentFetchAvailable: false,
    }))

    try {
      const { DashboardIntelligencePanel: EnabledDashboardIntelligencePanel } =
        await import('./panels')

      render(
        <MemoryRouter>
          <EnabledDashboardIntelligencePanel
            aiMeta={{ description: 'Provider ready', label: 'ready' } as never}
            backgroundQueueCount={3}
            embeddingProviderId="nomic"
            language="en"
            llmProviderId="llama"
            t={t}
          />
        </MemoryRouter>,
      )

      expect(screen.getByText('ready')).toBeVisible()
      expect(screen.getByText('Provider ready')).toBeVisible()
      expect(screen.getByText('3')).toBeVisible()
      expect(
        screen.getByRole('link', { name: 'dashboard.semanticSearchAction' }),
      ).toHaveAttribute('href', '/explorer?mode=hybrid')
      expect(
        screen.getByRole('link', { name: 'dashboard.openAssistantAction' }),
      ).toHaveAttribute('href', '/assistant')
    } finally {
      vi.doUnmock('../../lib/release-capabilities')
      vi.resetModules()
    }
  })

  test('renders On This Day loading and fallback states', () => {
    const { rerender } = render(
      <MemoryRouter>
        <DashboardOnThisDayPanel
          activeOnThisDay={[]}
          activeOnThisDayError={null}
          activeProfileId={null}
          intelligenceT={intelligenceT}
          onThisDayLoading
        />
      </MemoryRouter>,
    )

    expect(document.querySelector('.intelligence-stack')).toHaveAttribute(
      'aria-busy',
      'true',
    )

    rerender(
      <MemoryRouter>
        <DashboardOnThisDayPanel
          activeOnThisDay={[]}
          activeOnThisDayError="on-this-day failed"
          activeProfileId={null}
          intelligenceT={intelligenceT}
          onThisDayLoading={false}
        />
      </MemoryRouter>,
    )
    expect(screen.getByText('on-this-day failed')).toBeVisible()
  })

  test('renders On This Day entries and caps domain links', () => {
    render(
      <MemoryRouter>
        <DashboardOnThisDayPanel
          activeOnThisDay={[entryFixture()]}
          activeOnThisDayError={null}
          activeProfileId="chrome:Default"
          intelligenceT={intelligenceT}
          onThisDayLoading={false}
        />
      </MemoryRouter>,
    )

    expect(
      screen.getByRole('link', {
        name: '2025 · onThisDayVisits:{"count":12}',
      }),
    ).toHaveAttribute(
      'href',
      '/intelligence/day/2026-04-25?profileId=chrome%3ADefault',
    )
    expect(screen.getByText('Read local-first storage notes')).toBeVisible()
    expect(screen.getByRole('link', { name: 'sqlite.org' })).toHaveAttribute(
      'href',
      '/intelligence/domain/sqlite.org?range=custom&start=2026-04-25&end=2026-04-25&profileId=chrome%3ADefault',
    )
    expect(
      screen.queryByRole('link', { name: 'hidden.example' }),
    ).not.toBeInTheDocument()
  })

  test('renders On This Day entries without summary or domains', () => {
    render(
      <MemoryRouter>
        <DashboardOnThisDayPanel
          activeOnThisDay={[
            {
              ...entryFixture(),
              summary: null,
              topDomains: [],
            },
          ]}
          activeOnThisDayError={null}
          activeProfileId={null}
          intelligenceT={intelligenceT}
          onThisDayLoading={false}
        />
      </MemoryRouter>,
    )

    expect(
      screen.getByRole('link', {
        name: '2025 · onThisDayVisits:{"count":12}',
      }),
    ).toHaveAttribute('href', '/intelligence/day/2026-04-25')
    expect(
      screen.queryByText('Read local-first storage notes'),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByRole('link', { name: 'sqlite.org' }),
    ).not.toBeInTheDocument()
  })

  test('renders zero-state checklist completion variants', () => {
    const { rerender } = render(
      <MemoryRouter>
        <DashboardZeroStateChecklistPanel
          dashboard={dashboardFixture({ recentRuns: [] })}
          snapshotInitialized={false}
          t={t}
        />
      </MemoryRouter>,
    )

    expect(screen.getByText('1')).toHaveClass('dim')
    expect(screen.getByText('2')).toHaveClass('dim')

    rerender(
      <MemoryRouter>
        <DashboardZeroStateChecklistPanel
          dashboard={dashboardFixture({ recentRuns: [runFixture()] })}
          snapshotInitialized
          t={t}
        />
      </MemoryRouter>,
    )

    expect(screen.getAllByText('✓')).toHaveLength(2)
    expect(screen.getAllByText('✓')[0]).toHaveClass('accent')
  })
})

function dashboardFixture(
  overrides: Partial<DashboardSnapshot> = {},
): DashboardSnapshot {
  return {
    generatedAt: '2026-04-25T12:00:00Z',
    lastSuccessfulBackupAt: null,
    nextAction: null,
    recentRuns: [runFixture()],
    storage: {
      archiveDatabaseBytes: 0,
      exportBytes: 0,
      intelligenceBlobBytes: 0,
      intelligenceDatabaseBytes: 0,
      manifestBytes: 0,
      quarantineBytes: 0,
      searchDatabaseBytes: 0,
      semanticSidecarBytes: 0,
      snapshotBytes: 0,
      sourceEvidenceDatabaseBytes: 0,
      stagingBytes: 0,
    },
    totalDownloads: 0,
    totalProfiles: 0,
    totalUrls: 0,
    totalVisits: 0,
    ...overrides,
  }
}

function runFixture(
  overrides: Partial<BackupRunOverview> = {},
): BackupRunOverview {
  return {
    finishedAt: '2026-04-25T11:10:00Z',
    id: 8,
    manifestHash: null,
    newDownloads: 0,
    newUrls: 2,
    newVisits: 4,
    profileScope: ['chrome:Default'],
    profilesProcessed: 1,
    runType: 'backup',
    startedAt: '2026-04-25T11:00:00Z',
    status: 'success',
    trigger: 'manual',
    ...overrides,
  }
}

function browserProfileFixture(
  overrides: Partial<BrowserProfile> = {},
): BrowserProfile {
  return {
    accessIssue: null,
    browserFamily: 'chromium',
    browserName: 'Google Chrome',
    browserVersion: '124.0.0',
    faviconsBytes: 0,
    faviconsPath: null,
    historyBytes: 1024,
    historyExists: true,
    historyFileName: 'History',
    historyPath: '/Users/test/Chrome/Default/History',
    historyReadable: true,
    profileId: 'chrome:Default',
    profileName: 'Default',
    profilePath: '/Users/test/Chrome/Default',
    retentionBoundary: { kind: 'browser-managed', localDays: 90 },
    supportingBytes: 1024,
    userName: null,
    ...overrides,
  }
}

function entryFixture(): OnThisDayEntry {
  return {
    year: 2025,
    date: '2026-04-25',
    totalVisits: 12,
    deepDiveSessions: 1,
    summary: 'Read local-first storage notes',
    topDomains: [
      'sqlite.org',
      'tauri.app',
      'react.dev',
      'bun.sh',
      'hidden.example',
    ],
  }
}
