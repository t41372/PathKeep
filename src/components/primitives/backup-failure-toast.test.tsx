/**
 * Coverage for the backup-failure toast and its diagnostic-report builder.
 *
 * What this test owns:
 * - The pure `buildBackupDiagnosticReport` shape across present/absent build
 *   info, raw detail, and error kind.
 * - Component behavior the shell-level test does not exercise: focus-on-mount,
 *   the copy-to-clipboard success/blocked paths, the "Copied" auto-revert, and
 *   the technical-details disclosure.
 */

import { act, fireEvent, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '@/lib/i18n'
import type { AppBuildInfo } from '@/lib/types'
import {
  BackupFailureToast,
  type BackupFailureToastProps,
} from './backup-failure-toast'
import { buildBackupDiagnosticReport } from './backup-failure-diagnostics'

const buildInfo: AppBuildInfo = {
  productName: 'PathKeep',
  version: '0.3.0',
  gitCommitShort: 'abc1234',
  gitCommitFull: 'abc1234deadbeef',
  gitDirty: false,
}

function renderToast(overrides: Partial<BackupFailureToastProps> = {}) {
  const props: BackupFailureToastProps = {
    message: 'Backup failed: disk I/O error',
    rawError: 'sqlite: SQLITE_IOERR',
    errorKind: null,
    buildInfo,
    onRetry: vi.fn(),
    onDismiss: vi.fn(),
    onOpenFdaSettings: vi.fn(),
    onRevealLogs: vi.fn(),
    ...overrides,
  }
  render(
    <I18nProvider>
      <BackupFailureToast {...props} />
    </I18nProvider>,
  )
  return props
}

describe('buildBackupDiagnosticReport', () => {
  test('includes version, platform, kind, message, and raw detail', () => {
    const report = buildBackupDiagnosticReport({
      message: 'human message',
      rawError: 'raw backend text',
      errorKind: 'full-disk-access',
      buildInfo,
      timestamp: '2026-06-25T03:14:00.000Z',
      userAgent: 'TestAgent/1.0',
    })
    expect(report).toContain('PathKeep backup failure')
    expect(report).toContain('2026-06-25T03:14:00.000Z')
    expect(report).toContain('0.3.0 (abc1234deadbeef)')
    expect(report).toContain('TestAgent/1.0')
    expect(report).toContain('Kind:     full-disk-access')
    expect(report).toContain('Message:  human message')
    expect(report).toContain('Detail:   raw backend text')
  })

  test('falls back to unknown version, generic kind, and (none) detail when absent', () => {
    const report = buildBackupDiagnosticReport({
      message: 'm',
      rawError: null,
      errorKind: null,
      buildInfo: null,
      timestamp: 't',
      userAgent: 'ua',
    })
    expect(report).toContain('Version:  unknown')
    expect(report).toContain('Kind:     generic')
    expect(report).toContain('Detail:   (none)')
  })
})

describe('BackupFailureToast', () => {
  afterEach(() => {
    vi.useRealTimers()
    vi.restoreAllMocks()
  })

  test('moves focus to the alert on mount so keyboard users land on the actions', () => {
    renderToast()
    expect(screen.getByTestId('backup-failure-toast')).toHaveFocus()
  })

  test('shows the FDA remediation button only for the full-disk-access kind', async () => {
    const user = userEvent.setup()
    const { onOpenFdaSettings } = renderToast({ errorKind: 'full-disk-access' })
    const toast = screen.getByTestId('backup-failure-toast')
    await user.click(
      within(toast).getByRole('button', { name: /full disk access/i }),
    )
    expect(onOpenFdaSettings).toHaveBeenCalled()
  })

  test('omits the FDA button for a generic failure', () => {
    renderToast({ errorKind: null })
    const toast = screen.getByTestId('backup-failure-toast')
    expect(
      within(toast).queryByRole('button', { name: /full disk access/i }),
    ).not.toBeInTheDocument()
  })

  test('retry and dismiss fire their callbacks', async () => {
    const user = userEvent.setup()
    const { onRetry, onDismiss } = renderToast()
    const toast = screen.getByTestId('backup-failure-toast')
    await user.click(within(toast).getByRole('button', { name: /try again/i }))
    await user.click(within(toast).getByRole('button', { name: /dismiss/i }))
    expect(onRetry).toHaveBeenCalled()
    expect(onDismiss).toHaveBeenCalled()
  })

  test('reveals the raw diagnostic report inside the technical-details panel', async () => {
    const user = userEvent.setup()
    renderToast({ rawError: 'sqlite: SQLITE_IOERR' })
    const toast = screen.getByTestId('backup-failure-toast')
    await user.click(within(toast).getByText(/technical details/i))
    await user.click(
      within(toast).getByRole('button', { name: /logs folder/i }),
    )
    expect(toast).toHaveTextContent('sqlite: SQLITE_IOERR')
  })

  test('copy diagnostics confirms with "Copied" and reverts after the timeout', async () => {
    vi.useFakeTimers()
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText },
      configurable: true,
    })
    renderToast()
    const toast = screen.getByTestId('backup-failure-toast')
    await act(async () => {
      fireEvent.click(
        within(toast).getByRole('button', { name: /copy diagnostics/i }),
      )
      // Flush the Promise.resolve().then(writeText).then(setCopied) chain.
      await vi.advanceTimersByTimeAsync(0)
    })
    expect(writeText).toHaveBeenCalledWith(
      expect.stringContaining('Backup failed: disk I/O error'),
    )
    expect(
      within(toast).getByRole('button', { name: /^Copied$/ }),
    ).toBeInTheDocument()
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2100)
    })
    expect(
      within(toast).getByRole('button', { name: /copy diagnostics/i }),
    ).toBeInTheDocument()
  })

  test('copy diagnostics swallows a blocked-clipboard rejection without a second error', async () => {
    const user = userEvent.setup()
    const writeText = vi.fn().mockRejectedValue(new Error('clipboard blocked'))
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText },
      configurable: true,
    })
    renderToast()
    const toast = screen.getByTestId('backup-failure-toast')
    await user.click(
      within(toast).getByRole('button', { name: /copy diagnostics/i }),
    )
    await vi.waitFor(() => expect(writeText).toHaveBeenCalled())
    expect(
      within(toast).queryByRole('button', { name: /^Copied$/ }),
    ).not.toBeInTheDocument()
    expect(
      within(toast).getByRole('button', { name: /copy diagnostics/i }),
    ).toBeInTheDocument()
  })
})
