/**
 * @file remote-backup-section.test.tsx
 * @description Render-level coverage for the remote-backup PME review surface.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Verify preview/execute/verify actions and manual/warning rows.
 * - Protect the null-draft guard used while Settings route state hydrates.
 *
 * ## Not responsible for
 * - Re-testing persistent remote backup preference editing.
 * - Re-testing backend S3-compatible upload behavior.
 *
 * ## Dependencies
 * - Uses the shipped i18n provider and typed remote-backup DTO fixtures.
 *
 * ## Performance notes
 * - Renders one bounded Settings panel per state.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import type {
  RemoteBackupConfig,
  RemoteBackupPreview,
  RemoteBackupResult,
  RemoteBackupVerification,
} from '../../lib/types'
import {
  RemoteBackupSection,
  type RemoteBackupSectionProps,
  type RemoteBackupSectionState,
} from './remote-backup-section'
import type { SettingsSectionNavItem } from './section-nav-items'

const navItem: SettingsSectionNavItem = {
  id: 'settings-remote-pme',
  icon: 'cloud_upload',
  key: 'remote',
  label: 'Cloud backup',
}

describe('RemoteBackupSection', () => {
  test('does not render until the route draft has hydrated', () => {
    const { container } = renderSection({
      state: stateFixture({ currentDraft: null }),
    })

    expect(container.firstChild).toBeNull()
  })

  test('wires top-level PME actions and renders preview warnings', async () => {
    const user = userEvent.setup()
    const handlers = handlerFixture()
    renderSection({
      credentialsSaved: true,
      lastError: 'Last upload failed',
      lastUploadedAt: '2026-04-25T12:00:00Z',
      lastUploadedObjectKey: 'pathkeep/archive.zip',
      state: stateFixture({
        ...handlers,
        latestRemoteBundlePath: '/tmp/pathkeep-remote.zip',
        preview: previewFixture(),
      }),
    })

    await user.click(screen.getByRole('button', { name: 'Preview upload' }))
    await user.click(screen.getByRole('button', { name: 'Upload now' }))
    await user.click(screen.getByRole('button', { name: 'Verify backup' }))

    expect(handlers.onPreview).toHaveBeenCalledTimes(1)
    expect(handlers.onExecute).toHaveBeenCalledTimes(1)
    expect(handlers.onVerify).toHaveBeenCalledTimes(1)
    expect(screen.getByText('/tmp/pathkeep-remote.zip')).toBeVisible()
    expect(screen.getAllByText('pathkeep/archive.zip')).toHaveLength(2)
    expect(
      screen.getByText('Keep this bundle local until upload succeeds.'),
    ).toBeVisible()
    expect(screen.getByText('Last upload failed')).toBeVisible()
  })

  test('renders manual, execute, and verify payloads', () => {
    const { rerender } = renderSection({
      state: stateFixture({
        preview: previewFixture(),
        tab: 'manual',
      }),
    })

    expect(screen.getByText('aws s3 cp bundle.zip s3://pathkeep')).toBeVisible()
    expect(screen.getByText('Upload the bundle manually.')).toBeVisible()

    rerender(
      sectionNode({
        state: stateFixture({
          result: resultFixture(),
          tab: 'execute',
        }),
      }),
    )
    expect(screen.getByText('Uploaded archive bundle.')).toBeVisible()

    rerender(
      sectionNode({
        state: stateFixture({
          tab: 'verify',
          verification: verificationFixture(),
        }),
      }),
    )
    expect(screen.getByText('restore command ready')).toBeVisible()
    expect(screen.getByText('manifest')).toBeVisible()
  })

  test('renders empty draft fallbacks and not-run PME states', () => {
    const emptyDraft = draftFixture({
      bucket: '',
      enabled: false,
      endpoint: '',
      pathStyle: false,
      prefix: '',
      region: '',
      uploadAfterBackup: false,
    })
    const { rerender } = renderSection({
      state: stateFixture({
        action: 'Preparing remote backup',
        configured: false,
        currentDraft: emptyDraft,
      }),
    })

    expect(screen.getByText('Preparing remote backup')).toBeVisible()
    expect(screen.getByText('Not saved yet')).toBeVisible()
    expect(screen.getByText('No upload yet.')).toBeVisible()
    expect(screen.getAllByText('No').length).toBeGreaterThanOrEqual(3)
    expect(screen.getAllByText('Not available').length).toBeGreaterThanOrEqual(
      4,
    )
    expect(
      screen.getByRole('button', { name: 'Preview upload' }),
    ).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Upload now' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Verify backup' })).toBeDisabled()

    rerender(
      sectionNode({
        state: stateFixture({
          preview: null,
          tab: 'manual',
        }),
      }),
    )
    expect(screen.getByText('Preview first')).toBeVisible()

    rerender(
      sectionNode({
        state: stateFixture({
          result: null,
          tab: 'execute',
        }),
      }),
    )
    expect(screen.getByText('Not uploaded yet')).toBeVisible()

    rerender(
      sectionNode({
        state: stateFixture({
          result: {
            ...resultFixture(),
            message: 'Bundle created but upload skipped.',
            uploaded: false,
          },
          tab: 'execute',
        }),
      }),
    )
    expect(screen.getByText('Bundle created but upload skipped.')).toBeVisible()

    rerender(
      sectionNode({
        state: stateFixture({
          tab: 'verify',
          verification: null,
        }),
      }),
    )
    expect(screen.getByText('Nothing to verify yet')).toBeVisible()

    rerender(
      sectionNode({
        state: stateFixture({
          tab: 'verify',
          verification: {
            ...verificationFixture(),
            checks: [],
            restoreReady: false,
            restoreSteps: [],
          },
        }),
      }),
    )
    expect(screen.getByText('Needs attention')).toBeVisible()
  })
})

function renderSection(overrides: Partial<RemoteBackupSectionProps> = {}) {
  return render(sectionNode(overrides))
}

function sectionNode(overrides: Partial<RemoteBackupSectionProps> = {}) {
  const props: RemoteBackupSectionProps = {
    credentialsSaved: false,
    lastError: null,
    lastUploadedAt: null,
    lastUploadedObjectKey: null,
    navItem,
    state: stateFixture(),
    ...overrides,
  }

  return (
    <MemoryRouter>
      <I18nProvider>
        <RemoteBackupSection {...props} />
      </I18nProvider>
    </MemoryRouter>
  )
}

function stateFixture(
  overrides: Partial<RemoteBackupSectionState> = {},
): RemoteBackupSectionState {
  return {
    accessKeyId: 'access',
    action: null,
    configured: true,
    currentDraft: draftFixture(),
    latestRemoteBundlePath: null,
    preview: null,
    result: null,
    secretAccessKey: 'secret',
    tab: 'preview',
    verification: null,
    ...handlerFixture(),
    ...overrides,
  }
}

function handlerFixture() {
  return {
    onAccessKeyIdChange: vi.fn(),
    onClearCredentials: vi.fn().mockResolvedValue(undefined),
    onDraftChange: vi.fn(),
    onExecute: vi.fn().mockResolvedValue(undefined),
    onPreview: vi.fn().mockResolvedValue(undefined),
    onSaveConfig: vi.fn().mockResolvedValue(undefined),
    onSecretAccessKeyChange: vi.fn(),
    onSetTab: vi.fn(),
    onStoreCredentials: vi.fn().mockResolvedValue(undefined),
    onVerify: vi.fn().mockResolvedValue(undefined),
  }
}

function draftFixture(
  overrides: Partial<RemoteBackupConfig> = {},
): RemoteBackupConfig {
  return {
    bucket: 'pathkeep',
    credentialsSaved: false,
    enabled: true,
    endpoint: 'https://s3.example.test',
    lastError: null,
    lastUploadedAt: null,
    lastUploadedObjectKey: null,
    pathStyle: true,
    prefix: 'backups/',
    region: 'us-east-1',
    uploadAfterBackup: true,
    ...overrides,
  }
}

function previewFixture(): RemoteBackupPreview {
  return {
    bundlePath: '/tmp/pathkeep-remote.zip',
    objectKey: 'pathkeep/archive.zip',
    uploadUrl: 's3://pathkeep/archive.zip',
    previewCommand: 'aws s3 cp bundle.zip s3://pathkeep',
    manualSteps: ['Upload the bundle manually.'],
    warnings: ['Keep this bundle local until upload succeeds.'],
  }
}

function resultFixture(): RemoteBackupResult {
  return {
    uploaded: true,
    bundlePath: '/tmp/pathkeep-remote.zip',
    objectKey: 'pathkeep/archive.zip',
    uploadUrl: 's3://pathkeep/archive.zip',
    message: 'Uploaded archive bundle.',
  }
}

function verificationFixture(): RemoteBackupVerification {
  return {
    bundlePath: '/tmp/pathkeep-remote.zip',
    bundleVersion: '1',
    appVersion: '0.1.0',
    createdAt: '2026-04-25T12:00:00Z',
    archiveMode: 'Encrypted',
    objectKey: 'pathkeep/archive.zip',
    restoreReady: true,
    checks: [
      {
        name: 'manifest',
        status: 'ok',
        message: 'manifest verified',
      },
    ],
    warnings: [],
    restoreSteps: ['restore command ready'],
    manifestFiles: [],
  }
}
