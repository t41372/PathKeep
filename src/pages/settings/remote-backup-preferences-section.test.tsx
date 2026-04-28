import { fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import type { RemoteBackupConfig } from '../../lib/types'
import {
  RemoteBackupPreferencesSection,
  type RemoteBackupPreferencesSectionProps,
} from './remote-backup-preferences-section'
import type { RemoteBackupSectionState } from './remote-backup-section'
import type { SettingsSectionNavItem } from './section-nav-items'

const navItem: SettingsSectionNavItem = {
  id: 'settings-remote',
  icon: 'cloud_upload',
  key: 'remote',
  label: 'Cloud backup',
}

describe('RemoteBackupPreferencesSection', () => {
  test('does not render until the remote draft has hydrated', () => {
    const { container } = renderSection({
      state: stateFixture({ currentDraft: null }),
    })

    expect(container.firstChild).toBeNull()
  })

  test('wires preference fields and credential actions to route-owned handlers', () => {
    const handlers = handlerFixture()
    renderSection({
      credentialsSaved: true,
      lastError: 'Last upload failed',
      lastUploadedAt: '2026-04-25T12:00:00Z',
      lastUploadedObjectKey: 'pathkeep/archive.zip',
      state: stateFixture({
        ...handlers,
        accessKeyId: 'old-access',
        secretAccessKey: 'old-secret',
        currentDraft: draftFixture({
          enabled: true,
          pathStyle: false,
          uploadAfterBackup: true,
          endpoint: 'https://s3.example.test',
        }),
      }),
    })

    fireEvent.click(screen.getByLabelText('Enable cloud backup'))
    fireEvent.click(screen.getByLabelText('Use path-style URLs'))
    fireEvent.click(screen.getByLabelText('Auto-upload after each backup'))
    fireEvent.change(screen.getByLabelText('Bucket'), {
      target: { value: 'archive-bucket' },
    })
    fireEvent.change(screen.getByLabelText('Region'), {
      target: { value: 'us-west-2' },
    })
    fireEvent.change(screen.getByLabelText('Custom endpoint'), {
      target: { value: '' },
    })
    fireEvent.change(screen.getByLabelText('Path prefix'), {
      target: { value: 'daily/' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Save' }))
    fireEvent.change(screen.getByLabelText('Access key ID'), {
      target: { value: 'new-access' },
    })
    fireEvent.change(screen.getByLabelText('Secret access key'), {
      target: { value: 'new-secret' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Save credentials' }))
    fireEvent.click(screen.getByRole('button', { name: 'Remove credentials' }))

    expect(handlers.onDraftChange).toHaveBeenNthCalledWith(1, {
      enabled: false,
    })
    expect(handlers.onDraftChange).toHaveBeenNthCalledWith(2, {
      pathStyle: true,
    })
    expect(handlers.onDraftChange).toHaveBeenNthCalledWith(3, {
      uploadAfterBackup: false,
    })
    expect(handlers.onDraftChange).toHaveBeenNthCalledWith(4, {
      bucket: 'archive-bucket',
    })
    expect(handlers.onDraftChange).toHaveBeenNthCalledWith(5, {
      region: 'us-west-2',
    })
    expect(handlers.onDraftChange).toHaveBeenNthCalledWith(6, {
      endpoint: null,
    })
    expect(handlers.onDraftChange).toHaveBeenNthCalledWith(7, {
      prefix: 'daily/',
    })
    expect(handlers.onSaveConfig).toHaveBeenCalledTimes(1)
    expect(handlers.onAccessKeyIdChange).toHaveBeenCalledWith('new-access')
    expect(handlers.onSecretAccessKeyChange).toHaveBeenCalledWith('new-secret')
    expect(handlers.onStoreCredentials).toHaveBeenCalledTimes(1)
    expect(handlers.onClearCredentials).toHaveBeenCalledTimes(1)
    expect(screen.getByText('pathkeep/archive.zip')).toBeInTheDocument()
    expect(screen.getByText('Last upload failed')).toBeInTheDocument()
  })

  test('disables credential actions when input or saved state is missing', () => {
    renderSection({
      credentialsSaved: false,
      state: stateFixture({
        accessKeyId: '   ',
        secretAccessKey: 'secret',
      }),
    })

    expect(
      screen.getByRole('button', { name: 'Save credentials' }),
    ).toBeDisabled()
    expect(
      screen.getByRole('button', { name: 'Remove credentials' }),
    ).toBeDisabled()
    expect(screen.getByText('No upload yet.')).toBeInTheDocument()
  })
})

function renderSection(
  overrides: Partial<RemoteBackupPreferencesSectionProps> = {},
) {
  const props: RemoteBackupPreferencesSectionProps = {
    credentialsSaved: false,
    lastError: null,
    lastUploadedAt: null,
    lastUploadedObjectKey: null,
    navItem,
    state: stateFixture(),
    ...overrides,
  }

  return render(
    <MemoryRouter>
      <I18nProvider>
        <RemoteBackupPreferencesSection {...props} />
      </I18nProvider>
    </MemoryRouter>,
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
    enabled: false,
    endpoint: null,
    lastError: null,
    lastUploadedAt: null,
    lastUploadedObjectKey: null,
    pathStyle: true,
    prefix: 'backups/',
    region: 'us-east-1',
    uploadAfterBackup: false,
    ...overrides,
  }
}
