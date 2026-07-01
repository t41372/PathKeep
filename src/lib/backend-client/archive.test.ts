/**
 * @file archive.test.ts
 * @description Pins the archive client command names and argument shapes so transport
 * regressions surface as test failures rather than silent no-ops.
 * @module lib/backend-client
 *
 * ## Responsibilities
 * - Assert that `listRecoverySnapshots` routes to the correct command with no args.
 * - Assert that `runFullArchiveRestore` routes to the correct command with the right arg shape.
 *
 * ## Not responsible for
 * - End-to-end restore logic or snapshot fixture content.
 * - Browser-preview degradation (archive client uses the real transport only).
 *
 * ## Dependencies
 * - Uses the same hoisted mock pattern as `content-enrichment.test.ts`.
 */

import { beforeEach, describe, expect, test } from 'vitest'

const {
  invokeCommandMock,
  hasDesktopCommandTransportMock,
  backendHarnessMock,
} = vi.hoisted(() => ({
  invokeCommandMock: vi.fn(),
  hasDesktopCommandTransportMock: vi.fn(() => true),
  backendHarnessMock: {
    call: vi.fn(),
  },
}))

vi.mock('../ipc/bridge', () => ({
  invokeCommand: invokeCommandMock,
}))

vi.mock('../runtime', () => ({
  hasDesktopCommandTransport: hasDesktopCommandTransportMock,
}))

vi.mock('../backend', () => ({
  backendTestHarness: backendHarnessMock,
}))

describe('archive client', () => {
  beforeEach(() => {
    invokeCommandMock.mockReset()
    backendHarnessMock.call.mockReset()
    hasDesktopCommandTransportMock.mockReturnValue(true)
    ;(
      window as Window & {
        __PATHKEEP_DESKTOP_COMMAND_METRICS__?: unknown[]
      }
    ).__PATHKEEP_DESKTOP_COMMAND_METRICS__ = []
  })

  test('listRecoverySnapshots calls the correct command with no extra args', async () => {
    const snapshots = [
      {
        id: 'snap-1',
        path: '/snap.sqlite',
        createdAt: null,
        sizeBytes: 1024,
        verifiedOpenable: true,
        sourceOp: 'backup',
        label: 'Backup snapshot',
      },
    ]
    invokeCommandMock.mockResolvedValueOnce(snapshots)

    const { archiveClient } = await import('./archive')
    const result = await archiveClient.listRecoverySnapshots()

    expect(invokeCommandMock).toHaveBeenCalledWith(
      'list_recovery_snapshots',
      undefined,
    )
    expect(result).toEqual(snapshots)
  })

  test('runFullArchiveRestore calls the correct command with the snapshotPath request', async () => {
    const report = {
      runId: 42,
      restoredSnapshotPath: '/snap.sqlite',
      restoredMode: 'Plaintext',
      quarantineDir: '/quarantine',
      sourceEvidenceRebuilt: true,
      warnings: [],
    }
    invokeCommandMock.mockResolvedValueOnce(report)

    const { archiveClient } = await import('./archive')
    const result = await archiveClient.runFullArchiveRestore({
      snapshotPath: '/snap.sqlite',
    })

    expect(invokeCommandMock).toHaveBeenCalledWith('run_full_archive_restore', {
      request: { snapshotPath: '/snap.sqlite' },
    })
    expect(result).toEqual(report)
  })
})
