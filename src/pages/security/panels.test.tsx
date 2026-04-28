import { createRef } from 'react'
import { fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import type { RekeyPreview, SecurityStatus } from '../../lib/types'
import {
  SecurityRekeyPanel,
  SecurityStatusPanel,
  SecurityUnlockPanel,
} from './panels'

const reviewMocks = vi.hoisted(() => ({
  copyReviewValue: vi.fn(),
}))

vi.mock('../../components/review', () => ({
  copyReviewValue: reviewMocks.copyReviewValue,
  ReviewPathActionRow: ({
    copyKey,
    copyLabel,
    label,
    onCopy,
    onOpenPath,
    openPathLabel,
    value,
  }: {
    copyKey: string
    copyLabel: string
    label: string
    onCopy: (key: string, value: string) => void
    onOpenPath: (path: string) => void
    openPathLabel: string
    value: string
  }) => (
    <div data-testid={`path-row:${copyKey}`}>
      <span>{label}</span>
      <span>{value}</span>
      <button type="button" onClick={() => onCopy(copyKey, value)}>
        {copyLabel}:{copyKey}
      </button>
      <button type="button" onClick={() => onOpenPath(value)}>
        {openPathLabel}:{copyKey}
      </button>
    </div>
  ),
}))

describe('Security route panels', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  test('renders archive posture, warnings, audit links, and path actions', () => {
    const setCopyFeedback = vi.fn()
    const onOpenPath = vi.fn()

    render(
      <MemoryRouter>
        <SecurityStatusPanel
          copyFeedback={null}
          language="en"
          localizedWarnings={['Keyring unavailable']}
          onOpenPath={onOpenPath}
          setCopyFeedback={setCopyFeedback}
          status={securityStatusFixture({
            lastRekeyRunId: 42,
            lastRekeySnapshotPath: '/tmp/rekey.sqlite',
          })}
          t={testT}
        />
      </MemoryRouter>,
    )

    expect(screen.getByText('security.encryptionStatus')).toBeInTheDocument()
    expect(screen.getByText('security.passwordLossTitle')).toBeInTheDocument()
    expect(screen.getByText('Keyring unavailable')).toBeInTheDocument()
    expect(
      screen.getByRole('link', { name: 'security.openLastRekeyAudit' }),
    ).toHaveAttribute('href', '/audit?run=42')

    fireEvent.click(
      screen.getByRole('button', {
        name: 'common.copyAction:security:stronghold',
      }),
    )
    fireEvent.click(
      screen.getByRole('button', {
        name: 'common.copyAction:security:archive',
      }),
    )
    fireEvent.click(
      screen.getByRole('button', {
        name: 'common.copyAction:security:last-rekey-snapshot',
      }),
    )
    fireEvent.click(
      screen.getByRole('button', { name: 'common.openPath:security:archive' }),
    )
    fireEvent.click(
      screen.getByRole('button', {
        name: 'common.openAction:security:last-rekey-snapshot',
      }),
    )

    expect(reviewMocks.copyReviewValue).toHaveBeenCalledWith(
      '/tmp/vault.hold',
      {
        key: 'security:stronghold',
        onFeedback: setCopyFeedback,
      },
    )
    expect(reviewMocks.copyReviewValue).toHaveBeenCalledWith(
      '/tmp/history.sqlite',
      {
        key: 'security:archive',
        onFeedback: setCopyFeedback,
      },
    )
    expect(reviewMocks.copyReviewValue).toHaveBeenCalledWith(
      '/tmp/rekey.sqlite',
      {
        key: 'security:last-rekey-snapshot',
        onFeedback: setCopyFeedback,
      },
    )
    expect(onOpenPath).toHaveBeenCalledWith('/tmp/history.sqlite')
    expect(onOpenPath).toHaveBeenCalledWith('/tmp/rekey.sqlite')
  })

  test('wires locked, unlocked, and plaintext unlock/keyring controls', () => {
    const lockedHandlers = unlockHandlers()
    const unlockInputRef = createRef<HTMLInputElement>()
    const { rerender } = render(
      <SecurityUnlockPanel
        {...lockedHandlers}
        busy="security.unlockArchive"
        sessionKey="secret"
        status={securityStatusFixture({ unlocked: false })}
        t={testT}
        unlockInputRef={unlockInputRef}
      />,
    )

    fireEvent.change(screen.getByLabelText('security.currentDatabaseKey'), {
      target: { value: 'new-secret' },
    })
    fireEvent.click(
      screen.getByRole('button', { name: 'security.unlockArchive' }),
    )
    fireEvent.click(screen.getByRole('button', { name: 'security.useKeyring' }))
    fireEvent.click(
      screen.getByRole('button', { name: 'security.storeInKeyring' }),
    )
    fireEvent.click(
      screen.getByRole('button', { name: 'security.clearKeyring' }),
    )

    expect(lockedHandlers.setSessionKey).toHaveBeenCalledWith('new-secret')
    expect(lockedHandlers.handleUnlock).toHaveBeenCalledTimes(1)
    expect(lockedHandlers.handleUnlockFromKeyring).toHaveBeenCalledTimes(1)
    expect(lockedHandlers.handleStoreKeyringKey).toHaveBeenCalledTimes(1)
    expect(lockedHandlers.handleClearKeyring).toHaveBeenCalledTimes(1)

    const unlockedHandlers = unlockHandlers()
    rerender(
      <SecurityUnlockPanel
        {...unlockedHandlers}
        busy="security.lockArchive"
        sessionKey=""
        status={securityStatusFixture({ unlocked: true })}
        t={testT}
        unlockInputRef={unlockInputRef}
      />,
    )
    fireEvent.click(
      screen.getByRole('button', { name: 'security.lockArchive' }),
    )
    expect(unlockedHandlers.handleLockArchive).toHaveBeenCalledTimes(1)

    rerender(
      <SecurityUnlockPanel
        {...unlockHandlers()}
        busy={null}
        sessionKey=""
        status={securityStatusFixture({
          encrypted: false,
          mode: 'Plaintext',
          unlocked: true,
        })}
        t={testT}
        unlockInputRef={unlockInputRef}
      />,
    )
    expect(
      screen.queryByRole('button', { name: 'security.lockArchive' }),
    ).not.toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: 'security.storeInKeyring' }),
    ).toBeInTheDocument()
  })

  test('keeps busy labels on each keyring action branch', () => {
    const unlockInputRef = createRef<HTMLInputElement>()
    const { rerender } = render(
      <SecurityUnlockPanel
        {...unlockHandlers()}
        busy="security.useKeyring"
        sessionKey=""
        status={securityStatusFixture({ unlocked: false })}
        t={testT}
        unlockInputRef={unlockInputRef}
      />,
    )

    expect(
      screen.getByRole('button', { name: 'security.useKeyring' }),
    ).toBeInTheDocument()

    rerender(
      <SecurityUnlockPanel
        {...unlockHandlers()}
        busy="security.storeInKeyring"
        sessionKey=""
        status={securityStatusFixture({ unlocked: false })}
        t={testT}
        unlockInputRef={unlockInputRef}
      />,
    )
    expect(
      screen.getByRole('button', { name: 'security.storeInKeyring' }),
    ).toBeInTheDocument()

    rerender(
      <SecurityUnlockPanel
        {...unlockHandlers()}
        busy="security.clearKeyring"
        sessionKey=""
        status={securityStatusFixture({ unlocked: false })}
        t={testT}
        unlockInputRef={unlockInputRef}
      />,
    )
    expect(
      screen.getByRole('button', { name: 'security.clearKeyring' }),
    ).toBeInTheDocument()
  })

  test('wires rekey preview, mode changes, confirmation, warnings, and execution', () => {
    const handlers = rekeyHandlers()
    const { rerender } = render(
      <SecurityRekeyPanel
        {...handlers}
        actionError="Could not rekey"
        busy="security.previewRekey"
        localizedWarning={(warning) => `localized:${warning}`}
        notice="Preview ready"
        preview={rekeyPreviewFixture()}
        rekeyConfirmText=""
        rekeyKey="new-key"
        rekeyMode="Encrypted"
        saveRekeyKey
        t={testT}
      />,
    )

    fireEvent.change(screen.getByLabelText('security.targetMode'), {
      target: { value: 'Plaintext' },
    })
    fireEvent.change(screen.getByLabelText('security.newDatabaseKey'), {
      target: { value: 'newer-key' },
    })
    fireEvent.click(screen.getByLabelText('security.storeNewKey'))
    fireEvent.click(
      screen.getByRole('button', { name: 'security.previewRekey' }),
    )
    fireEvent.click(
      screen.getByRole('button', { name: 'security.executeRekey' }),
    )

    expect(handlers.setPreview).toHaveBeenCalledWith(null)
    expect(handlers.setRekeyConfirmText).toHaveBeenCalledWith('')
    expect(handlers.setRekeyMode).toHaveBeenCalledWith('Plaintext')
    expect(handlers.setRekeyKey).toHaveBeenCalledWith('newer-key')
    expect(handlers.setSaveRekeyKey).toHaveBeenCalledWith(false)
    expect(handlers.handlePreviewRekey).toHaveBeenCalledTimes(1)
    expect(handlers.handleExecuteRekey).toHaveBeenCalledTimes(1)
    expect(screen.getByText('/tmp/rekey-snapshot.sqlite')).toBeInTheDocument()
    expect(screen.getByText('localized:Back up first')).toBeInTheDocument()
    expect(screen.getByRole('alert')).toHaveTextContent('Could not rekey')

    const plaintextHandlers = rekeyHandlers()
    rerender(
      <SecurityRekeyPanel
        {...plaintextHandlers}
        actionError={null}
        busy={null}
        localizedWarning={(warning) => warning}
        notice={null}
        preview={rekeyPreviewFixture({
          currentMode: 'Encrypted',
          nextMode: 'Plaintext',
        })}
        rekeyConfirmText=""
        rekeyKey=""
        rekeyMode="Plaintext"
        saveRekeyKey={false}
        t={testT}
      />,
    )
    expect(
      screen.getByRole('button', { name: 'security.executeRekey' }),
    ).toBeDisabled()
    fireEvent.change(screen.getByLabelText('security.rekeyConfirmLabel'), {
      target: { value: 'confirm' },
    })
    expect(plaintextHandlers.setRekeyConfirmText).toHaveBeenCalledWith(
      'confirm',
    )

    rerender(
      <SecurityRekeyPanel
        {...plaintextHandlers}
        actionError={null}
        busy="security.executeRekey"
        localizedWarning={(warning) => warning}
        notice={null}
        preview={rekeyPreviewFixture({
          currentMode: 'Encrypted',
          nextMode: 'Plaintext',
        })}
        rekeyConfirmText="confirm"
        rekeyKey=""
        rekeyMode="Plaintext"
        saveRekeyKey={false}
        t={testT}
      />,
    )
    fireEvent.click(
      screen.getByRole('button', { name: 'security.executeRekey' }),
    )
    expect(plaintextHandlers.handleExecuteRekey).toHaveBeenCalledTimes(1)
  })
})

function testT(key: string, vars?: Record<string, string | number>) {
  if (key === 'security.archiveIs') {
    return `Archive is ${vars?.mode}`
  }
  return key
}

function unlockHandlers() {
  return {
    handleClearKeyring: vi.fn().mockResolvedValue(undefined),
    handleLockArchive: vi.fn().mockResolvedValue(undefined),
    handleStoreKeyringKey: vi.fn().mockResolvedValue(undefined),
    handleUnlock: vi.fn().mockResolvedValue(undefined),
    handleUnlockFromKeyring: vi.fn().mockResolvedValue(undefined),
    setSessionKey: vi.fn(),
  }
}

function rekeyHandlers() {
  return {
    handleExecuteRekey: vi.fn().mockResolvedValue(undefined),
    handlePreviewRekey: vi.fn().mockResolvedValue(undefined),
    setPreview: vi.fn(),
    setRekeyConfirmText: vi.fn(),
    setRekeyKey: vi.fn(),
    setRekeyMode: vi.fn(),
    setSaveRekeyKey: vi.fn(),
  }
}

function securityStatusFixture(
  overrides: Partial<SecurityStatus> = {},
): SecurityStatus {
  return {
    databasePath: '/tmp/history.sqlite',
    encrypted: true,
    initialized: true,
    keyringStatus: {
      available: true,
      backend: 'stronghold',
      message: null,
      storedSecret: true,
    },
    lastRekeyAt: '2026-04-25T12:00:00Z',
    lastRekeyRunId: null,
    lastRekeySnapshotPath: null,
    lastSuccessfulBackupAt: '2026-04-24T12:00:00Z',
    mode: 'Encrypted',
    rememberDatabaseKeyInKeyring: true,
    strongholdPath: '/tmp/vault.hold',
    unlocked: false,
    warnings: [],
    ...overrides,
  }
}

function rekeyPreviewFixture(
  overrides: Partial<RekeyPreview> = {},
): RekeyPreview {
  return {
    currentMode: 'Plaintext',
    nextMode: 'Encrypted',
    requiresNewKey: true,
    snapshotPath: '/tmp/rekey-snapshot.sqlite',
    steps: ['Create snapshot', 'Rewrite database'],
    tempDatabasePath: '/tmp/rekey-temp.sqlite',
    warnings: ['Back up first'],
    ...overrides,
  }
}
