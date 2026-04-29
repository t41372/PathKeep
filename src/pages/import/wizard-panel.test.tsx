/**
 * @file wizard-panel.test.tsx
 * @description Focused render coverage for Import wizard and source-selection callbacks.
 * @module pages/import
 *
 * ## Responsibilities
 * - Verify preview-step back/import actions remain connected to the route owner.
 * - Verify the Takeout folder picker passes the directory intent through.
 *
 * ## Not responsible for
 * - Running backend import mutations.
 * - Re-testing the full Import route flow.
 *
 * ## Dependencies
 * - Uses the real i18n provider so button labels stay tied to shipping copy.
 *
 * ## Performance notes
 * - Direct component rendering avoids the larger trust-flow harness for small callback contracts.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import type { ShellTask } from '../../app/shell-tasks'
import { I18nProvider, createNamespaceTranslator } from '../../lib/i18n'
import type { BrowserProfile, TakeoutInspection } from '../../lib/types'
import { ImportSelectStep } from './select-step'
import type { ImportWizardPanelProps } from './wizard-panel'
import { ImportWizardPanel } from './wizard-panel'

const importT = createNamespaceTranslator('en', 'import')

describe('Import wizard render modules', () => {
  test('routes preview back and confirm actions to the owner callbacks', async () => {
    const user = userEvent.setup()
    const onImport = vi.fn().mockResolvedValue(undefined)
    const onStepChange = vi.fn()

    renderWizard({
      inspection: inspectionFixture(),
      onImport,
      onStepChange,
      step: 'preview',
      stepIndex: 2,
    })

    await user.click(
      screen.getByRole('button', { name: importT('backAction') }),
    )
    expect(onStepChange).toHaveBeenCalledWith('select')

    await user.click(
      screen.getByRole('button', { name: importT('confirmImport') }),
    )
    expect(onImport).toHaveBeenCalled()
  })

  test('renders Takeout mismatch guidance, file dispositions, and zero-import guardrails', () => {
    const htmlInspection = inspectionFixture({
      candidateItems: 0,
      duplicateItems: 0,
      previewRangeEnd: null,
      previewRangeStart: null,
      quarantinedFiles: [
        {
          classification: 'needs-review',
          detectedLocale: 'zh-TW',
          kind: 'my-activity-html',
          path: '/Users/test/Takeout/My Activity/Chrome/MyActivity.html',
          reasonCode: 'chrome-my-activity-html',
          reasonDetail: 'HTML My Activity export is not a Chrome History file.',
          records: 0,
          status: 'needs-review',
        },
        {
          classification: 'known-but-ignored',
          detectedLocale: 'en',
          kind: 'outside-scope',
          path: '/Users/test/Takeout/Bookmarks.json',
          reasonCode: null,
          reasonDetail: null,
          records: 0,
          status: 'ignored',
        },
      ],
      recognizedFiles: [],
    })

    const htmlView = renderWizard({
      inspection: htmlInspection,
      step: 'preview',
      stepIndex: 2,
    })

    expect(
      screen.getByText(importT('takeoutMismatchDetectedTitle')),
    ).toBeVisible()
    expect(screen.getByText(importT('takeoutMismatchHtmlBody'))).toBeVisible()
    expect(
      screen.getByText(
        'This is a Chrome My Activity HTML export. PathKeep does not import HTML activity files in this build.',
      ),
    ).toBeVisible()
    expect(screen.getByText(importT('noImportableFilesNotice'))).toBeVisible()
    expect(
      screen.getByRole('button', { name: importT('confirmImport') }),
    ).toBeDisabled()
    htmlView.unmount()

    renderWizard({
      inspection: inspectionFixture({
        quarantinedFiles: [
          {
            classification: 'needs-review',
            detectedLocale: 'en',
            kind: 'my-activity-json',
            path: '/Users/test/Takeout/My Activity/Chrome/MyActivity.json',
            reasonCode: 'chrome-my-activity-json',
            reasonDetail: null,
            records: 2,
            status: 'needs-review',
          },
        ],
      }),
      step: 'preview',
      stepIndex: 2,
    })

    expect(screen.getByText(importT('takeoutMismatchJsonBody'))).toBeVisible()
    expect(
      screen.getByText(importT('fileRecordsLabel', { count: '2' })),
    ).toBeVisible()
  })

  test('renders confirm importing fallback progress before a progress event exists', () => {
    renderWizard({
      importing: true,
      inspection: null,
      importTask: null,
      step: 'confirm',
      stepIndex: 3,
    })

    expect(screen.getByText(importT('importingTitle'))).toBeVisible()
    expect(
      screen.getByText(
        importT('importingProgressDetail', { records: '0', files: '0' }),
      ),
    ).toBeVisible()
  })

  test('renders global task progress when confirm import has a shell task', () => {
    renderWizard({
      importing: true,
      importTask: taskFixture(),
      step: 'confirm',
      stepIndex: 3,
    })

    expect(screen.getByRole('heading', { name: 'Import Chrome' })).toBeVisible()
    expect(screen.getAllByText('3 / 12 records')).toHaveLength(2)
    expect(screen.getByRole('link', { name: 'Open Jobs' })).toHaveAttribute(
      'href',
      '/jobs',
    )
  })

  test('passes directory intent from the Takeout folder picker', async () => {
    const user = userEvent.setup()
    const onBrowseSource = vi.fn()

    render(
      <I18nProvider>
        <ImportSelectStep
          detectedBrowserProfiles={[]}
          language="en"
          manualPathExpanded={false}
          method="takeout"
          selectedBrowserProfile={null}
          selectedBrowserProfileId={null}
          sourcePath="/Users/test/Takeout"
          onBrowseSource={onBrowseSource}
          onManualPathExpandedChange={vi.fn()}
          onMethodChange={vi.fn()}
          onOpenFullDiskAccessSettings={vi.fn()}
          onScan={vi.fn()}
          onSelectBrowserProfile={vi.fn()}
          onSourcePathChange={vi.fn()}
        />
      </I18nProvider>,
    )

    await user.click(
      screen.getByRole('button', { name: importT('chooseTakeoutFolder') }),
    )
    expect(onBrowseSource).toHaveBeenCalledWith({ directory: true })
  })

  test('renders browser profile readiness, path, and Full Disk Access branches', async () => {
    const user = userEvent.setup()
    const onOpenFullDiskAccessSettings = vi.fn()
    const onSelectBrowserProfile = vi.fn()
    const readyProfile = browserProfileFixture({
      historyPath: '/profiles/Ready/History',
      profileId: 'chrome:ready',
      profileName: 'Ready',
      profilePath: '/profiles/Ready',
    })
    const readyView = render(
      <I18nProvider>
        <ImportSelectStep
          detectedBrowserProfiles={[readyProfile]}
          language="en"
          manualPathExpanded={false}
          method="browser"
          selectedBrowserProfile={readyProfile}
          selectedBrowserProfileId="chrome:ready"
          sourcePath="/profiles/Ready/History"
          onBrowseSource={vi.fn()}
          onManualPathExpandedChange={vi.fn()}
          onMethodChange={vi.fn()}
          onOpenFullDiskAccessSettings={onOpenFullDiskAccessSettings}
          onScan={vi.fn()}
          onSelectBrowserProfile={onSelectBrowserProfile}
          onSourcePathChange={vi.fn()}
        />
      </I18nProvider>,
    )

    expect(screen.getAllByText('History').length).toBeGreaterThan(0)
    const sourcePath = screen.getByText('/profiles/Ready/History')
    expect(sourcePath).not.toBeVisible()
    await user.click(screen.getByText(importT('browserProfileSourcePath')))
    expect(sourcePath).toBeVisible()
    await user.click(
      screen.getByRole('button', { name: 'Google Chrome · Ready' }),
    )
    expect(onSelectBrowserProfile).toHaveBeenCalledWith(
      expect.objectContaining({ profileId: 'chrome:ready' }),
    )
    readyView.unmount()

    render(
      <I18nProvider>
        <ImportSelectStep
          detectedBrowserProfiles={[
            browserProfileFixture({
              browserFamily: 'safari',
              browserName: 'Safari',
              historyPath: null,
              historyReadable: false,
              profileId: 'safari:personal',
              profileName: 'Personal',
              profilePath: '/profiles/Safari',
            }),
            browserProfileFixture({
              browserFamily: 'chromium',
              browserName: 'Google Chrome',
              historyReadable: false,
              profileId: 'chrome:locked',
              profileName: 'Locked',
            }),
          ]}
          language="en"
          manualPathExpanded={false}
          method="browser"
          selectedBrowserProfile={null}
          selectedBrowserProfileId="safari:personal"
          sourcePath=""
          onBrowseSource={vi.fn()}
          onManualPathExpandedChange={vi.fn()}
          onMethodChange={vi.fn()}
          onOpenFullDiskAccessSettings={onOpenFullDiskAccessSettings}
          onScan={vi.fn()}
          onSelectBrowserProfile={vi.fn()}
          onSourcePathChange={vi.fn()}
        />
      </I18nProvider>,
    )

    expect(screen.getByText(importT('safariFullDiskAccessHint'))).toBeVisible()
    expect(screen.getByText(importT('browserProfileUnreadable'))).toBeVisible()
    expect(screen.queryByText('/profiles/Safari')).not.toBeInTheDocument()
    await user.click(
      screen.getByRole('button', {
        name: importT('openFullDiskAccessSettings'),
      }),
    )
    expect(onOpenFullDiskAccessSettings).toHaveBeenCalledTimes(1)
  })
})

function renderWizard(overrides: Partial<ImportWizardPanelProps> = {}) {
  return render(
    <I18nProvider>
      <MemoryRouter>
        <ImportWizardPanel {...wizardProps(overrides)} />
      </MemoryRouter>
    </I18nProvider>,
  )
}

function wizardProps(
  overrides: Partial<ImportWizardPanelProps> = {},
): ImportWizardPanelProps {
  return {
    detectedBrowserProfiles: [],
    importing: false,
    importTask: null,
    importResult: null,
    inspection: null,
    language: 'en',
    manualPathExpanded: false,
    method: 'takeout',
    selectedBrowserProfile: null,
    selectedBrowserProfileId: null,
    sourcePath: '/Users/test/Takeout',
    step: 'select',
    stepIndex: 0,
    wizardSteps: [
      { key: 'select', label: importT('stepUpload') },
      { key: 'scan', label: importT('stepScan') },
      { key: 'preview', label: importT('stepPreview') },
      { key: 'confirm', label: importT('stepConfirm') },
      { key: 'done', label: importT('stepImport') },
    ],
    onBrowseSource: vi.fn(),
    onImport: vi.fn(),
    onImportAnother: vi.fn(),
    onManualPathExpandedChange: vi.fn(),
    onMethodChange: vi.fn(),
    onOpenFullDiskAccessSettings: vi.fn(),
    onScan: vi.fn(),
    onSelectBrowserProfile: vi.fn(),
    onSourcePathChange: vi.fn(),
    onStepChange: vi.fn(),
    ...overrides,
  }
}

function taskFixture(): ShellTask {
  return {
    id: 'task-import',
    kind: 'import',
    state: 'running',
    title: 'Import Chrome',
    detail: 'Writing archive records',
    startedAt: '2026-04-27T10:00:00.000Z',
    updatedAt: '2026-04-27T10:01:00.000Z',
    finishedAt: null,
    progressLabel: '3 / 12',
    progressValue: 25,
    processedRecords: 3,
    totalRecords: 12,
    logEntries: [],
  }
}

function inspectionFixture(
  overrides: Partial<TakeoutInspection> = {},
): TakeoutInspection {
  return {
    dryRun: true,
    sourcePath: '/Users/test/Takeout',
    recognizedFiles: [
      {
        classification: 'will-import',
        detectedLocale: 'en',
        kind: 'browser-json',
        path: '/Users/test/Takeout/BrowserHistory.json',
        reasonCode: 'chrome-history-json',
        reasonDetail: null,
        records: 12,
        status: 'previewed',
      },
    ],
    quarantinedFiles: [
      {
        classification: 'known-but-ignored',
        detectedLocale: 'en',
        kind: 'outside-scope',
        path: '/Users/test/Takeout/Bookmarks.json',
        reasonCode: 'outside-chrome-scope',
        reasonDetail: null,
        records: 0,
        status: 'ignored',
      },
    ],
    previewEntries: [],
    candidateItems: 12,
    importedItems: 0,
    duplicateItems: 2,
    notes: [],
    importBatch: null,
    detectedLocale: 'en',
    previewRangeStart: '2026-04-01T00:00:00.000Z',
    previewRangeEnd: '2026-04-02T00:00:00.000Z',
    ...overrides,
  }
}

function browserProfileFixture(
  overrides: Partial<BrowserProfile> = {},
): BrowserProfile {
  return {
    appDisplayName: 'Google Chrome',
    browserFamily: 'chromium',
    browserName: 'Google Chrome',
    faviconsPath: '/profiles/Default/Favicons',
    historyExists: true,
    historyPath: '/profiles/Default/History',
    historyReadable: true,
    lastVisitedAt: null,
    profileId: 'chrome:Default',
    profileName: 'Default',
    profilePath: '/profiles/Default',
    ...overrides,
  } as BrowserProfile
}
