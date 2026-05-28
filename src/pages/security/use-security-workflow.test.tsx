/**
 * @file use-security-workflow.test.tsx
 * @description Hook-level regressions for the Security route workflow owner.
 * @module pages/security
 *
 * ## Responsibilities
 * - Verify archive unlock, keyring, lock, and rekey handlers call the real backend facade contracts.
 * - Protect action-error and notice state so Security cannot silently report success after failed native work.
 * - Keep the Security route shell out of these tests; the hook is the behavior owner.
 *
 * ## Not responsible for
 * - Re-testing Security panel rendering or localized copy layout.
 * - Re-testing backend command implementations.
 *
 * ## Dependencies
 * - Uses backend-client spies and a paint-yield mock so async action state is deterministic.
 *
 * ## Performance notes
 * - The tests stay hook-level to cover native workflow branches without mounting the full app shell.
 */

import { act, renderHook, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend-client'
import { mockSnapshot } from '../../lib/backend-preview-fixtures'
import type {
  KeyringStatusReport,
  RekeyPreview,
  SecurityStatus,
} from '../../lib/types'
import { useSecurityWorkflow } from './use-security-workflow'

vi.mock('../../lib/wait-for-next-paint', () => ({
  waitForNextPaint: vi.fn(() => Promise.resolve()),
}))

const t = (key: string) => key

describe('useSecurityWorkflow', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  test('loads posture and rejects invalid or unverifiable unlock attempts', async () => {
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    const clearSessionKey = vi
      .spyOn(backend, 'clearSessionDatabaseKey')
      .mockResolvedValue(undefined)
    const setSessionKey = vi
      .spyOn(backend, 'setSessionDatabaseKey')
      .mockResolvedValue(undefined)
    vi.spyOn(backend, 'securityStatus')
      .mockResolvedValueOnce(securityStatusFixture({ unlocked: false }))
      .mockResolvedValue(securityStatusFixture({ unlocked: false }))

    const { result } = renderHook(() =>
      useSecurityWorkflow({
        refreshAppData,
        refreshKey: 1,
        t,
      }),
    )

    await waitFor(() => expect(result.current.status).not.toBeNull())
    expect(result.current.rekeyMode).toBe('Plaintext')

    await act(async () => {
      await result.current.handleUnlock()
    })
    expect(result.current.actionError).toBe(
      'security.currentDatabaseKeyRequired',
    )
    expect(setSessionKey).not.toHaveBeenCalled()

    act(() => {
      result.current.setSessionKey('  bad-key  ')
    })
    clearSessionKey.mockRejectedValueOnce(new Error('clear failed'))
    await act(async () => {
      await result.current.handleUnlock()
    })

    expect(setSessionKey).toHaveBeenCalledWith('bad-key')
    expect(clearSessionKey).toHaveBeenCalledTimes(1)
    expect(result.current.actionError).toBe('security.archiveUnlockFailed')
    expect(refreshAppData).not.toHaveBeenCalled()
  })

  test('executes unlock, keyring, store, clear, and lock workflows', async () => {
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    const securityStatus = vi
      .spyOn(backend, 'securityStatus')
      .mockResolvedValue(securityStatusFixture({ unlocked: true }))
    const setSessionKey = vi
      .spyOn(backend, 'setSessionDatabaseKey')
      .mockResolvedValue(undefined)
    const getKeyringKey = vi
      .spyOn(backend, 'keyringGetDatabaseKey')
      .mockResolvedValue('stored-key')
    const storeKeyringKey = vi
      .spyOn(backend, 'keyringStoreDatabaseKey')
      .mockResolvedValue(keyringStatusFixture({ storedSecret: true }))
    const clearKeyringKey = vi
      .spyOn(backend, 'keyringClearDatabaseKey')
      .mockResolvedValue(keyringStatusFixture({ storedSecret: false }))
    const clearSessionKey = vi
      .spyOn(backend, 'clearSessionDatabaseKey')
      .mockResolvedValue(undefined)

    const { result } = renderHook(() =>
      useSecurityWorkflow({
        refreshAppData,
        refreshKey: 1,
        t,
      }),
    )

    await waitFor(() => expect(result.current.status?.unlocked).toBe(true))

    act(() => {
      result.current.setSessionKey('  current-key  ')
    })
    await act(async () => {
      await result.current.handleUnlock()
    })
    expect(setSessionKey).toHaveBeenCalledWith('current-key')
    expect(result.current.sessionKey).toBe('')
    expect(result.current.notice).toBe('security.sessionUnlocked')

    await act(async () => {
      await result.current.handleUnlockFromKeyring()
    })
    expect(getKeyringKey).toHaveBeenCalledTimes(1)
    expect(setSessionKey).toHaveBeenLastCalledWith('stored-key')

    await act(async () => {
      await result.current.handleStoreKeyringKey()
    })
    expect(result.current.actionError).toBe(
      'security.currentDatabaseKeyRequired',
    )

    act(() => {
      result.current.setSessionKey(' fresh-key ')
    })
    await act(async () => {
      await result.current.handleStoreKeyringKey()
    })
    expect(storeKeyringKey).toHaveBeenCalledWith('fresh-key')
    expect(result.current.notice).toBe('security.storeInKeyring')

    await act(async () => {
      await result.current.handleClearKeyring()
    })
    expect(clearKeyringKey).toHaveBeenCalledTimes(1)
    expect(result.current.notice).toBe('security.clearKeyring')

    await act(async () => {
      await result.current.handleLockArchive()
    })
    expect(clearSessionKey).toHaveBeenCalledTimes(1)
    expect(result.current.notice).toBe('security.sessionLocked')
    expect(refreshAppData).toHaveBeenCalledTimes(5)
    expect(securityStatus).toHaveBeenCalled()
  })

  test('reports keyring misses and executes encrypted and plaintext rekeys', async () => {
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    vi.spyOn(backend, 'securityStatus').mockResolvedValue(
      securityStatusFixture({ unlocked: true }),
    )
    vi.spyOn(backend, 'keyringGetDatabaseKey').mockResolvedValue(null)
    const previewRekey = vi
      .spyOn(backend, 'previewRekeyArchive')
      .mockResolvedValue(rekeyPreviewFixture())
    const rekeyArchive = vi
      .spyOn(backend, 'rekeyArchive')
      .mockResolvedValue(structuredClone(mockSnapshot))
    const storeKeyringKey = vi
      .spyOn(backend, 'keyringStoreDatabaseKey')
      .mockResolvedValue(keyringStatusFixture({ storedSecret: true }))
    const clearKeyringKey = vi
      .spyOn(backend, 'keyringClearDatabaseKey')
      .mockResolvedValue(keyringStatusFixture({ storedSecret: false }))

    const { result } = renderHook(() =>
      useSecurityWorkflow({
        refreshAppData,
        refreshKey: 1,
        t,
      }),
    )

    await waitFor(() => expect(result.current.status?.initialized).toBe(true))

    await act(async () => {
      await result.current.handleUnlockFromKeyring()
    })
    expect(result.current.actionError).toBe('platform.keyringTitle')

    act(() => {
      result.current.setRekeyMode('Encrypted')
    })
    await act(async () => {
      await result.current.handleExecuteRekey()
    })
    expect(result.current.actionError).toBe('security.newDatabaseKeyRequired')
    expect(rekeyArchive).not.toHaveBeenCalled()

    act(() => {
      result.current.setRekeyKey('new-key')
      result.current.setRekeyConfirmText('new-key')
      result.current.setSaveRekeyKey(true)
    })
    await act(async () => {
      await result.current.handlePreviewRekey()
    })
    expect(previewRekey).toHaveBeenCalledWith({
      newMode: 'Encrypted',
      newKey: 'new-key',
    })
    expect(result.current.preview?.nextMode).toBe('Encrypted')

    await act(async () => {
      await result.current.handleExecuteRekey()
    })
    expect(rekeyArchive).toHaveBeenCalledWith({
      newMode: 'Encrypted',
      newKey: 'new-key',
    })
    expect(storeKeyringKey).toHaveBeenCalledWith('new-key')
    expect(result.current.preview).toBeNull()
    expect(result.current.rekeyKey).toBe('')
    expect(result.current.rekeyConfirmText).toBe('')

    act(() => {
      result.current.setRekeyMode('Plaintext')
    })
    await act(async () => {
      await result.current.handleExecuteRekey()
    })
    expect(rekeyArchive).toHaveBeenLastCalledWith({
      newMode: 'Plaintext',
      newKey: null,
    })
    expect(clearKeyringKey).toHaveBeenCalledTimes(1)
  })

  test('surfaces security load failures and skips rekey actions before initialization', async () => {
    const previewRekey = vi.spyOn(backend, 'previewRekeyArchive')
    const rekeyArchive = vi.spyOn(backend, 'rekeyArchive')
    vi.spyOn(backend, 'securityStatus')
      .mockRejectedValueOnce('offline')
      .mockResolvedValue(securityStatusFixture({ initialized: false }))

    const { result, rerender } = renderHook(
      ({ refreshKey }: { refreshKey: number }) =>
        useSecurityWorkflow({
          refreshAppData: vi.fn().mockResolvedValue(undefined),
          refreshKey,
          t,
        }),
      {
        initialProps: { refreshKey: 1 },
      },
    )

    await waitFor(() => expect(result.current.loadState.error).toBe('offline'))

    rerender({ refreshKey: 2 })
    await waitFor(() => expect(result.current.status?.initialized).toBe(false))

    await act(async () => {
      await result.current.handlePreviewRekey()
      await result.current.handleExecuteRekey()
    })

    expect(previewRekey).not.toHaveBeenCalled()
    expect(rekeyArchive).not.toHaveBeenCalled()
  })

  test('ignores late security load results after cleanup', async () => {
    const firstLoad = deferred<SecurityStatus>()
    vi.spyOn(backend, 'securityStatus').mockReturnValueOnce(firstLoad.promise)

    const first = renderHook(() =>
      useSecurityWorkflow({
        refreshAppData: vi.fn().mockResolvedValue(undefined),
        refreshKey: 1,
        t,
      }),
    )
    first.unmount()

    await act(async () => {
      firstLoad.resolve(securityStatusFixture({ unlocked: true }))
      await firstLoad.promise
    })
    expect(first.result.current.loadState.status).toBeNull()

    const failedLoad = deferred<SecurityStatus>()
    vi.spyOn(backend, 'securityStatus').mockReturnValueOnce(failedLoad.promise)
    const failed = renderHook(() =>
      useSecurityWorkflow({
        refreshAppData: vi.fn().mockResolvedValue(undefined),
        refreshKey: 1,
        t,
      }),
    )
    failed.unmount()

    await act(async () => {
      failedLoad.reject(new Error('late load failed'))
      await failedLoad.promise.catch(() => undefined)
    })
    expect(failed.result.current.loadState.error).toBeNull()
  })

  test('uses Error load messages, fallback action errors, and plaintext reload mode', async () => {
    const refreshAppData = vi.fn().mockResolvedValue(undefined)
    const securityStatus = vi
      .spyOn(backend, 'securityStatus')
      .mockRejectedValueOnce(new Error('security hard fail'))
      .mockResolvedValueOnce(securityStatusFixture({ encrypted: true }))
      .mockResolvedValueOnce(
        securityStatusFixture({ encrypted: false, mode: 'Plaintext' }),
      )
    vi.spyOn(backend, 'keyringStoreDatabaseKey').mockRejectedValueOnce(
      'store failed',
    )

    const { result, rerender } = renderHook(
      ({ refreshKey }: { refreshKey: number }) =>
        useSecurityWorkflow({
          refreshAppData,
          refreshKey,
          t,
        }),
      {
        initialProps: { refreshKey: 1 },
      },
    )

    await waitFor(() =>
      expect(result.current.loadState.error).toBe('security hard fail'),
    )

    rerender({ refreshKey: 2 })
    await waitFor(() => expect(result.current.status?.encrypted).toBe(true))
    act(() => {
      result.current.setSessionKey(' current-key ')
    })
    await act(async () => {
      await result.current.handleStoreKeyringKey()
    })
    expect(result.current.actionError).toBe('store failed')

    vi.spyOn(backend, 'keyringStoreDatabaseKey').mockResolvedValueOnce(
      keyringStatusFixture({ storedSecret: true }),
    )
    act(() => {
      result.current.setSessionKey(' current-key ')
    })
    await act(async () => {
      await result.current.handleStoreKeyringKey()
    })

    expect(securityStatus).toHaveBeenCalledTimes(3)
    expect(result.current.rekeyMode).toBe('Encrypted')
    expect(result.current.notice).toBe('security.storeInKeyring')
  })
})

function securityStatusFixture(
  overrides: Partial<SecurityStatus> = {},
): SecurityStatus {
  return {
    initialized: true,
    mode: 'Encrypted',
    encrypted: true,
    unlocked: true,
    databasePath: '/Users/test/pathkeep/history-vault.sqlite',
    strongholdPath: '/Users/test/pathkeep/stronghold',
    rememberDatabaseKeyInKeyring: false,
    lastSuccessfulBackupAt: null,
    lastRekeyAt: null,
    lastRekeyRunId: null,
    lastRekeySnapshotPath: null,
    keyringStatus: {
      available: true,
      backend: 'file-backed-test',
      storedSecret: false,
    },
    warnings: [],
    ...overrides,
  }
}

function rekeyPreviewFixture(): RekeyPreview {
  return {
    currentMode: 'Plaintext',
    nextMode: 'Encrypted',
    requiresNewKey: true,
    snapshotPath: '/Users/test/pathkeep/rekey-snapshot.sqlite',
    tempDatabasePath: '/Users/test/pathkeep/rekey-temp.sqlite',
    steps: ['copy archive', 'verify archive'],
    warnings: [],
  }
}

function keyringStatusFixture(
  overrides: Partial<KeyringStatusReport> = {},
): KeyringStatusReport {
  return {
    available: true,
    backend: 'file-backed-test',
    storedSecret: false,
    ...overrides,
  }
}

function deferred<T>() {
  let resolve: (value: T) => void = () => {}
  let reject: (error: unknown) => void = () => {}
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })

  return { promise, reject, resolve }
}
