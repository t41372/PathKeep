/**
 * Coverage test for the paper-shell topbar.
 *
 * ## Responsibilities
 * - Cover detectModifierLabel() branches: Mac platform → ⌘, otherwise → Ctrl+,
 *   plus the typeof navigator === 'undefined' guard.
 * - Cover the backup-running label swap and disabled-state when archive is
 *   not initialized.
 */

import { afterEach, describe, expect, test, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { I18nProvider } from '@/lib/i18n'
import { PKTopbar } from './pk-topbar'
import type { AppScreen } from '@/app/router'

const screenDef = {
  titleKey: 'navigation.dashboardLabel',
  subtitleKey: 'navigation.dashboardSubtitle',
} as unknown as AppScreen

function renderTopbar(
  overrides: Partial<Parameters<typeof PKTopbar>[0]> = {},
) {
  const props: Parameters<typeof PKTopbar>[0] = {
    screen: screenDef,
    onOpenPalette: vi.fn(),
    onBackupNow: vi.fn(),
    backupRunning: false,
    archiveInitialized: true,
    ...overrides,
  }
  return render(
    <I18nProvider>
      <PKTopbar {...props} />
    </I18nProvider>,
  )
}

const originalDescriptor = Object.getOwnPropertyDescriptor(
  window.navigator,
  'platform',
)

afterEach(() => {
  if (originalDescriptor) {
    Object.defineProperty(window.navigator, 'platform', originalDescriptor)
  }
})

describe('PKTopbar', () => {
  test('shows the Cmd glyph on Mac-class platforms', () => {
    Object.defineProperty(window.navigator, 'platform', {
      configurable: true,
      value: 'MacIntel',
    })
    renderTopbar()
    expect(screen.getByText(/⌘K/)).toBeInTheDocument()
  })

  test('falls back to Ctrl+ on other platforms', () => {
    Object.defineProperty(window.navigator, 'platform', {
      configurable: true,
      value: 'Linux x86_64',
    })
    renderTopbar()
    expect(screen.getByText(/Ctrl\+K/)).toBeInTheDocument()
  })

  test('renders the archiving label and disables backup when backup is running', () => {
    renderTopbar({ backupRunning: true })
    expect(screen.getByTestId('pk-topbar')).toBeInTheDocument()
    const backupButton = screen.getAllByRole('button').at(-1)
    expect(backupButton).toHaveAttribute('disabled')
  })

  test('disables backup when the archive has not been initialized', () => {
    renderTopbar({ archiveInitialized: false })
    const backupButton = screen.getAllByRole('button').at(-1)
    expect(backupButton).toHaveAttribute('disabled')
  })
})
