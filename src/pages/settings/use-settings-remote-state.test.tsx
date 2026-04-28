import { act, renderHook } from '@testing-library/react'
import type { ReactNode } from 'react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { mockSnapshot } from '../../lib/backend-preview-fixtures'
import { I18nProvider } from '../../lib/i18n'
import type {
  AppConfig,
  AppSnapshot,
  RemoteBackupPreview,
  RemoteBackupResult,
  RemoteBackupVerification,
} from '../../lib/types'
import { useSettingsRemoteState } from './use-settings-remote-state'

function Wrapper({ children }: { children: ReactNode }) {
  return <I18nProvider>{children}</I18nProvider>
}

describe('useSettingsRemoteState', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  test('returns a no-op remote model until a snapshot is available', async () => {
    const storeCredentials = vi.spyOn(backend, 'storeS3Credentials')
    const verifyRemoteBackup = vi.spyOn(backend, 'verifyRemoteBackup')
    const { result } = renderHook(
      () =>
        useSettingsRemoteState({
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          saveConfig: vi.fn(),
          snapshot: null,
        }),
      { wrapper: Wrapper },
    )

    expect(result.current.remote.currentDraft).toBeNull()
    expect(result.current.remote.configured).toBe(false)

    await act(async () => {
      result.current.remote.onDraftChange({ bucket: 'ignored' })
      await result.current.remote.onSaveConfig()
      await result.current.remote.onStoreCredentials()
      await result.current.remote.onVerify()
    })

    expect(result.current.remote.currentDraft).toBeNull()
    expect(storeCredentials).not.toHaveBeenCalled()
    expect(verifyRemoteBackup).not.toHaveBeenCalled()
  })

  test('persists config, credentials, preview, execute, and verify workflows', async () => {
    const snapshot = snapshotFixture()
    const savedSnapshot = snapshotFixture({
      remoteBackup: {
        ...snapshot.config.remoteBackup,
        bucket: 'saved-bucket',
        credentialsSaved: true,
      },
    })
    const saveConfig = vi.fn((config: AppConfig) =>
      Promise.resolve({
        ...savedSnapshot,
        config: {
          ...savedSnapshot.config,
          ...config,
          remoteBackup: {
            ...config.remoteBackup,
            bucket: 'saved-bucket',
            credentialsSaved: true,
          },
        },
      }),
    )
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    const storeCredentials = vi
      .spyOn(backend, 'storeS3Credentials')
      .mockResolvedValue(undefined)
    const clearCredentials = vi
      .spyOn(backend, 'clearS3Credentials')
      .mockResolvedValue(undefined)
    vi.spyOn(backend, 'previewRemoteBackup').mockResolvedValue(previewFixture())
    vi.spyOn(backend, 'runRemoteBackup').mockResolvedValue(resultFixture())
    vi.spyOn(backend, 'verifyRemoteBackup').mockResolvedValue(
      verificationFixture(),
    )

    const { result } = renderHook(
      () =>
        useSettingsRemoteState({
          refreshAppData,
          saveConfig,
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    expect(result.current.remote.configured).toBe(true)

    act(() => {
      result.current.remote.onDraftChange({ bucket: 'draft-bucket' })
      result.current.remote.onAccessKeyIdChange('  access-key  ')
      result.current.remote.onSecretAccessKeyChange('  secret-key  ')
    })

    expect(result.current.remote.currentDraft?.bucket).toBe('draft-bucket')

    await act(async () => {
      await result.current.remote.onSaveConfig()
    })
    expect(saveConfig).toHaveBeenLastCalledWith(
      expect.objectContaining({
        remoteBackup: expect.objectContaining({
          bucket: 'draft-bucket',
          credentialsSaved: false,
        }),
      }),
    )
    expect(result.current.remote.currentDraft?.bucket).toBe('saved-bucket')

    await act(async () => {
      await result.current.remote.onStoreCredentials()
    })
    expect(storeCredentials).toHaveBeenCalledWith({
      accessKeyId: 'access-key',
      secretAccessKey: 'secret-key',
    })
    expect(result.current.remote.accessKeyId).toBe('')
    expect(result.current.remote.secretAccessKey).toBe('')
    expect(refreshAppData).toHaveBeenCalledTimes(1)

    await act(async () => {
      await result.current.remote.onClearCredentials()
    })
    expect(clearCredentials).toHaveBeenCalledTimes(1)
    expect(refreshAppData).toHaveBeenCalledTimes(2)

    await act(async () => {
      await result.current.remote.onPreview()
    })
    expect(result.current.remote.preview?.objectKey).toBe('archive.zip')
    expect(result.current.remote.tab).toBe('preview')

    await act(async () => {
      await result.current.remote.onExecute()
    })
    expect(result.current.remote.result?.bundlePath).toBe('/tmp/archive.zip')
    expect(result.current.remote.latestRemoteBundlePath).toBe(
      '/tmp/archive.zip',
    )
    expect(result.current.remote.tab).toBe('execute')
    expect(refreshAppData).toHaveBeenCalledTimes(3)

    await act(async () => {
      await result.current.remote.onVerify()
    })
    expect(backend.verifyRemoteBackup).toHaveBeenCalledWith('/tmp/archive.zip')
    expect(result.current.remote.verification?.checks[0]?.name).toBe('sha256')
    expect(result.current.remote.tab).toBe('verify')
    expect(result.current.remote.action).toBeNull()
  })

  test('ignores blank credential submissions and verify requests without a bundle', async () => {
    const snapshot = snapshotFixture()
    const storeCredentials = vi.spyOn(backend, 'storeS3Credentials')
    const verifyRemoteBackup = vi.spyOn(backend, 'verifyRemoteBackup')
    const { result } = renderHook(
      () =>
        useSettingsRemoteState({
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          saveConfig: vi.fn((config: AppConfig) =>
            Promise.resolve({
              ...snapshot,
              config,
            }),
          ),
          snapshot,
        }),
      { wrapper: Wrapper },
    )

    await act(async () => {
      result.current.remote.onAccessKeyIdChange('  ')
      result.current.remote.onSecretAccessKeyChange('secret')
      await result.current.remote.onStoreCredentials()
      await result.current.remote.onVerify()
    })

    expect(storeCredentials).not.toHaveBeenCalled()
    expect(verifyRemoteBackup).not.toHaveBeenCalled()
  })

  test('keeps a local draft but refuses persistence after the snapshot disappears', async () => {
    const snapshot = snapshotFixture()
    const saveConfig = vi.fn((config: AppConfig) =>
      Promise.resolve({
        ...snapshot,
        config,
      }),
    )
    const initialProps: { currentSnapshot: AppSnapshot | null } = {
      currentSnapshot: snapshot,
    }
    const { rerender, result } = renderHook(
      ({ currentSnapshot }: { currentSnapshot: AppSnapshot | null }) =>
        useSettingsRemoteState({
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          saveConfig,
          snapshot: currentSnapshot,
        }),
      {
        initialProps,
        wrapper: Wrapper,
      },
    )

    rerender({ currentSnapshot: null })

    await act(async () => {
      await result.current.remote.onSaveConfig()
    })

    expect(result.current.remote.currentDraft?.bucket).toBe('pathkeep')
    expect(saveConfig).not.toHaveBeenCalled()
  })
})

function snapshotFixture(configPatch: Partial<AppConfig> = {}): AppSnapshot {
  return {
    ...mockSnapshot,
    config: {
      ...mockSnapshot.config,
      ...configPatch,
      remoteBackup: {
        ...mockSnapshot.config.remoteBackup,
        enabled: true,
        bucket: 'pathkeep',
        region: 'us-east-1',
        ...configPatch.remoteBackup,
      },
    },
  }
}

function previewFixture(): RemoteBackupPreview {
  return {
    bundlePath: '/tmp/archive.zip',
    manualSteps: ['upload archive.zip'],
    objectKey: 'archive.zip',
    previewCommand: 'curl --upload-file archive.zip',
    uploadUrl: 's3://pathkeep/archive.zip',
    warnings: [],
  }
}

function resultFixture(): RemoteBackupResult {
  return {
    bundlePath: '/tmp/archive.zip',
    message: 'Uploaded archive.zip',
    objectKey: 'archive.zip',
    uploaded: true,
    uploadUrl: 's3://pathkeep/archive.zip',
  }
}

function verificationFixture(): RemoteBackupVerification {
  return {
    appVersion: '0.1.0',
    archiveMode: 'encrypted',
    bundlePath: '/tmp/archive.zip',
    bundleVersion: 'remote-v1',
    checks: [
      {
        name: 'sha256',
        message: 'Verified',
        status: 'ok',
      },
    ],
    createdAt: '2026-04-25T12:00:00Z',
    manifestFiles: [
      {
        relativePath: 'archive.zip',
        sha256: 'abc123',
        sizeBytes: 1024,
      },
    ],
    objectKey: 'archive.zip',
    restoreReady: true,
    restoreSteps: ['download archive.zip'],
    warnings: [],
  }
}
