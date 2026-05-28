/**
 * @file external-output-local-host-panel.test.tsx
 * @description Settings trusted local-host output flow coverage.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Verify preview loading/error, manual artifact actions, build success, build failure, and verify actions.
 * - Protect the local-only file URL and file-manager boundaries.
 * - Keep ready=false as a no-op render path.
 *
 * ## Not responsible for
 * - Re-testing localized fallback mapping; `helpers.test.ts` owns that.
 * - Re-testing the shared generated artifact viewer beyond this panel's callbacks.
 *
 * ## Dependencies
 * - Uses real `useAsyncData` and mocked Core Intelligence/desktop backend boundaries.
 *
 * ## Performance notes
 * - Small fixtures cover the panel contract without invoking the real filesystem or backend.
 */

import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import type * as coreIntelligenceModule from '../../lib/core-intelligence'
import type {
  DateRange,
  IntelligenceInstalledLocalHost,
  IntelligenceLocalHostBuildResult,
  IntelligenceLocalHostPreview,
} from '../../lib/core-intelligence'
import { backend } from '../../lib/backend-client'
import { SettingsExternalOutputLocalHostPanel } from './external-output-local-host-panel'

const { buildIntelligenceLocalHostMock, previewIntelligenceLocalHostMock } =
  vi.hoisted(() => ({
    buildIntelligenceLocalHostMock: vi.fn(),
    previewIntelligenceLocalHostMock: vi.fn(),
  }))

vi.mock('../../lib/core-intelligence', async (importOriginal) => {
  const actual = await importOriginal<typeof coreIntelligenceModule>()

  return {
    ...actual,
    buildIntelligenceLocalHost: buildIntelligenceLocalHostMock,
    previewIntelligenceLocalHost: previewIntelligenceLocalHostMock,
  }
})

vi.mock('../../lib/i18n/hooks', () => ({
  useI18n: () => ({
    language: 'en',
    ns: () => (key: string) => key,
  }),
}))

vi.mock('../../lib/backend-client', () => ({
  backend: {
    openExternalUrl: vi.fn().mockResolvedValue(undefined),
    openPathInFileManager: vi.fn().mockResolvedValue(undefined),
  },
}))

const dateRange: DateRange = {
  start: '2026-04-01',
  end: '2026-04-30',
}

describe('SettingsExternalOutputLocalHostPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: {
        writeText: vi.fn().mockResolvedValue(undefined),
      },
    })
  })

  test('renders nothing while the surrounding settings state is not ready', async () => {
    const { container } = renderLocalHostPanel({ ready: false })

    await act(async () => {
      await Promise.resolve()
    })

    expect(container).toBeEmptyDOMElement()
    expect(previewIntelligenceLocalHostMock).not.toHaveBeenCalled()
  })

  test('shows loading, then unavailable state with a refresh action', async () => {
    let resolvePreview: (
      preview: IntelligenceLocalHostPreview,
    ) => void = () => {}
    previewIntelligenceLocalHostMock.mockReturnValueOnce(
      new Promise<IntelligenceLocalHostPreview>((resolve) => {
        resolvePreview = resolve
      }),
    )

    const { rerender } = renderLocalHostPanel()

    expect(
      await screen.findByText('externalOutputsLocalHostLoading'),
    ).toBeVisible()

    act(() => {
      resolvePreview(createPreview({ generatedFiles: [], manualSteps: [] }))
    })

    expect(
      await screen.findByText('externalOutputsLocalHostVerifyUnavailable'),
    ).toBeVisible()

    previewIntelligenceLocalHostMock.mockRejectedValueOnce(
      new Error('preview unavailable'),
    )
    rerender(
      <SettingsExternalOutputLocalHostPanel
        activeProfileId="chrome:Default"
        dateRange={{ start: '2026-05-01', end: '2026-05-31' }}
        ready
      />,
    )

    expect(await screen.findByText('preview unavailable')).toBeVisible()

    previewIntelligenceLocalHostMock.mockResolvedValueOnce(createPreview())
    await userEvent.click(screen.getByRole('button', { name: 'refreshAction' }))

    await waitFor(() => {
      expect(previewIntelligenceLocalHostMock).toHaveBeenCalledTimes(3)
    })
  })

  test('builds a local host and exposes verify/open/copy actions', async () => {
    const user = userEvent.setup()
    previewIntelligenceLocalHostMock.mockResolvedValue(createPreview())
    buildIntelligenceLocalHostMock.mockResolvedValue(createBuildResult())

    renderLocalHostPanel({ activeProfileId: 'chrome:Default' })

    expect(
      await screen.findByText('externalOutputsLocalHostManualReview'),
    ).toBeVisible()
    await user.click(screen.getByRole('button', { name: 'openPath' }))
    expect(backend.openPathInFileManager).toHaveBeenCalledWith(
      '/tmp/pathkeep/browser-snippet-v1/index.html',
    )

    await user.click(
      screen.getByRole('button', {
        name: 'externalOutputsLocalHostCreateAction',
      }),
    )

    expect(buildIntelligenceLocalHostMock).toHaveBeenCalledWith(
      dateRange,
      'en',
      'chrome:Default',
    )
    expect(
      await screen.findByText('externalOutputsLocalHostBuilt'),
    ).toBeVisible()
    expect(screen.getByText('relative/pathkeep host/index.html')).toBeVisible()

    await user.click(
      screen.getByRole('button', {
        name: 'externalOutputsLocalHostOpenAction',
      }),
    )
    expect(backend.openExternalUrl).toHaveBeenCalledWith(
      'file:///relative/pathkeep%20host/index.html',
    )

    await user.click(screen.getByRole('button', { name: 'openDirectory' }))
    expect(backend.openPathInFileManager).toHaveBeenCalledWith(
      '/tmp/pathkeep/built-host',
    )

    const copyButtons = screen.getAllByRole('button', { name: 'copyAction' })
    await user.click(copyButtons[copyButtons.length - 1])
    expect(await screen.findByRole('status')).toHaveTextContent('copiedNotice')
  })

  test('surfaces non-error build failures with the underlying message', async () => {
    const user = userEvent.setup()
    previewIntelligenceLocalHostMock.mockResolvedValue(
      createPreview({ installedHost: createInstalledHost() }),
    )
    buildIntelligenceLocalHostMock.mockRejectedValue('nope')

    renderLocalHostPanel()

    await user.click(
      await screen.findByRole('button', {
        name: 'externalOutputsLocalHostUpdateAction',
      }),
    )

    expect(await screen.findByText('nope')).toBeVisible()
  })

  test('covers fallback preview, build, error, and timestamp branches', async () => {
    const user = userEvent.setup()
    previewIntelligenceLocalHostMock.mockResolvedValueOnce(
      null as unknown as IntelligenceLocalHostPreview,
    )
    const { rerender } = renderLocalHostPanel()

    expect(
      await screen.findByText('externalOutputsLocalHostUnavailableBody'),
    ).toBeVisible()

    previewIntelligenceLocalHostMock.mockResolvedValueOnce(
      createPreview({
        installedHost: createInstalledHost({
          bundle: createBundle({
            generatedAt: 'not-a-date',
            profileId: 'chrome:Default',
          }),
        }),
      }),
    )
    rerender(
      <SettingsExternalOutputLocalHostPanel
        activeProfileId={null}
        dateRange={{ start: '2026-06-01', end: '2026-06-30' }}
        ready
      />,
    )
    expect(await screen.findByText('not-a-date')).toBeVisible()
    expect(screen.getByText('Default')).toBeVisible()

    previewIntelligenceLocalHostMock.mockResolvedValueOnce(createPreview())
    buildIntelligenceLocalHostMock.mockResolvedValueOnce({
      ...createBuildResult(),
      installedHost: null,
    })
    await user.click(
      screen.getByRole('button', {
        name: 'externalOutputsLocalHostUpdateAction',
      }),
    )
    expect(
      await screen.findByText('externalOutputsLocalHostBuilt'),
    ).toBeVisible()

    previewIntelligenceLocalHostMock.mockResolvedValueOnce(createPreview())
    rerender(
      <SettingsExternalOutputLocalHostPanel
        activeProfileId={null}
        dateRange={{ start: '2026-07-01', end: '2026-07-31' }}
        ready
      />,
    )
    await screen.findByRole('button', {
      name: 'externalOutputsLocalHostCreateAction',
    })
    buildIntelligenceLocalHostMock.mockRejectedValueOnce(
      new Error('disk denied'),
    )
    await user.click(
      screen.getByRole('button', {
        name: 'externalOutputsLocalHostCreateAction',
      }),
    )
    expect(await screen.findByText('disk denied')).toBeVisible()
  })
})

function renderLocalHostPanel({
  activeProfileId = null,
  ready = true,
}: {
  activeProfileId?: string | null
  ready?: boolean
} = {}) {
  return render(
    <SettingsExternalOutputLocalHostPanel
      activeProfileId={activeProfileId}
      dateRange={dateRange}
      ready={ready}
    />,
  )
}

function createPreview(
  overrides: Partial<IntelligenceLocalHostPreview> = {},
): IntelligenceLocalHostPreview {
  return {
    artifactRoot: '/tmp/pathkeep/browser-snippet-v1',
    entryFilePath: '/tmp/pathkeep/browser-snippet-v1/index.html',
    generatedFiles: [
      {
        relativePath: 'index.html',
        absolutePath: '/tmp/pathkeep/browser-snippet-v1/index.html',
        purpose:
          'Core Intelligence snippet that can be opened directly in a local browser.',
        contents: '<!doctype html>',
      },
    ],
    bundle: createBundle({ profileId: null }),
    boundaryNotes: [
      'This local host only uses deterministic Core Intelligence read models.',
    ],
    manualSteps: [
      'Review index.html and bundle.json before handing this folder to another trusted local tool.',
    ],
    warnings: [
      'This local snippet includes trusted-only cards and should not be treated like a public export.',
    ],
    installedHost: null,
    ...overrides,
  }
}

function createBuildResult(): IntelligenceLocalHostBuildResult {
  const installedHost = createInstalledHost({
    artifactRoot: '/tmp/pathkeep/built-host',
    entryFilePath: 'relative/pathkeep host/index.html',
  })

  return {
    ...createPreview({
      artifactRoot: installedHost.artifactRoot,
      entryFilePath: installedHost.entryFilePath,
      installedHost,
    }),
    installedHost,
  }
}

function createInstalledHost(
  overrides: Partial<IntelligenceInstalledLocalHost> = {},
): IntelligenceInstalledLocalHost {
  return {
    artifactRoot: '/tmp/pathkeep/browser-snippet-v1',
    entryFilePath: '/tmp/pathkeep/browser-snippet-v1/index.html',
    bundle: createBundle({ profileId: null }),
    ...overrides,
  }
}

function createBundle({
  generatedAt = '2026-04-18T10:15:00Z',
  profileId,
}: {
  generatedAt?: string
  profileId: string | null
}): IntelligenceLocalHostPreview['bundle'] {
  return {
    bundleVersion: 'pathkeep.core-intelligence.local-host.v1',
    hostId: 'browser-snippet-v1',
    generatedAt,
    locale: 'en',
    dateRange,
    profileId,
    embedCards: [],
    widgetSnapshot: {
      generatedAt: '2026-04-18T10:15:00Z',
      dateRange,
      digestSummary: {
        dateRange,
        totalVisits: { value: 0, trend: 'flat' },
        totalSearches: { value: 0, trend: 'flat' },
        newDomains: { value: 0, trend: 'flat' },
        deepReadPages: { value: 0, trend: 'flat' },
        refindPages: { value: 0, trend: 'flat' },
      },
      highlights: [],
      notes: [],
    },
    publicSnapshot: {
      generatedAt: '2026-04-18T10:15:00Z',
      dateRange,
      digestSummary: {
        dateRange,
        totalVisits: { value: 0, trend: 'flat' },
        totalSearches: { value: 0, trend: 'flat' },
        newDomains: { value: 0, trend: 'flat' },
        deepReadPages: { value: 0, trend: 'flat' },
        refindPages: { value: 0, trend: 'flat' },
      },
      topDomains: [],
      searchEngines: [],
      discoveryTrend: { availableYears: [], points: [] },
      notes: [],
    },
    trustedOnlyCardIds: [],
    trustedOnlyCardCount: 0,
    boundaryNotes: [],
  }
}
