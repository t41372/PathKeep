/**
 * @file platform-section.test.tsx
 * @description Render-only coverage for Settings platform troubleshooting callouts.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Verify Safari, keyring, and scheduler guidance render from route-owned support snapshots.
 * - Keep repair links wired to the right product surfaces.
 *
 * ## Not responsible for
 * - Re-testing platform-guidance pure helpers.
 * - Re-testing the support-state hook that fetches these snapshots.
 *
 * ## Dependencies
 * - Uses the real i18n provider and MemoryRouter because this section renders route links.
 *
 * ## Performance notes
 * - Uses static fixtures only; the section is render-only and never starts background work.
 */

import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test } from 'vitest'
import { mockSnapshot } from '../../lib/backend-preview-fixtures'
import { I18nProvider } from '../../lib/i18n'
import type {
  AppSnapshot,
  ScheduleStatus,
  SecurityStatus,
} from '../../lib/types'
import type { SettingsSectionNavItem } from './section-nav-items'
import { PlatformSection } from './platform-section'

const navItem: SettingsSectionNavItem = {
  id: 'settings-platform',
  icon: 'settings',
  key: 'platform',
  label: 'Platform',
}

function renderSection({
  snapshot = structuredClone(mockSnapshot),
  scheduleStatus = scheduleFixture(),
  securityStatus = securityFixture(),
}: {
  snapshot?: AppSnapshot
  scheduleStatus?: ScheduleStatus | null
  securityStatus?: SecurityStatus | null
} = {}) {
  return render(
    <I18nProvider>
      <MemoryRouter>
        <PlatformSection
          navItem={navItem}
          snapshot={snapshot}
          supportState={{ scheduleStatus, securityStatus }}
        />
      </MemoryRouter>
    </I18nProvider>,
  )
}

function scheduleFixture(
  overrides: Partial<ScheduleStatus> = {},
): ScheduleStatus {
  return {
    platform: 'macos',
    label: 'com.yi-ting.pathkeep.backup',
    dueAfterHours: 72,
    checkIntervalHours: 6,
    applySupported: true,
    installState: 'manual-review',
    detectedFiles: [],
    manualSteps: [],
    auditPath: null,
    lastSuccessfulBackupAt: null,
    warnings: [],
    ...overrides,
  }
}

function securityFixture(
  overrides: Partial<SecurityStatus> = {},
): SecurityStatus {
  return {
    initialized: true,
    mode: 'Encrypted',
    encrypted: true,
    unlocked: true,
    databasePath: '/Users/test/pathkeep/history-vault.sqlite',
    strongholdPath: '/Users/test/pathkeep/vault.hold',
    rememberDatabaseKeyInKeyring: false,
    lastSuccessfulBackupAt: null,
    lastRekeyAt: null,
    lastRekeyRunId: null,
    lastRekeySnapshotPath: null,
    keyringStatus: {
      available: false,
      backend: 'macos-keychain',
      storedSecret: false,
    },
    warnings: ['Keychain unavailable'],
    ...overrides,
  }
}

describe('PlatformSection', () => {
  test('renders Safari access, keyring, and scheduler repair callouts', () => {
    const snapshot = structuredClone(mockSnapshot)
    snapshot.browserProfiles = snapshot.browserProfiles.map((profile) =>
      profile.browserFamily === 'safari'
        ? { ...profile, historyReadable: false }
        : profile,
    )

    renderSection({ snapshot })

    expect(screen.getByText('Safari needs Full Disk Access')).toBeVisible()
    expect(screen.getByText('System keychain not available')).toBeVisible()
    expect(screen.getByText('Schedule needs review')).toBeVisible()
    expect(screen.getByRole('link', { name: 'Imports' })).toHaveAttribute(
      'href',
      '/import',
    )
    expect(screen.getByRole('link', { name: 'Security' })).toHaveAttribute(
      'href',
      '/security',
    )
    expect(
      screen.getAllByRole('link', { name: 'Schedule' })[0],
    ).toHaveAttribute('href', '/schedule')
  })

  test('hides optional platform warnings when support snapshots are healthy', () => {
    const snapshot = structuredClone(mockSnapshot)
    renderSection({
      snapshot,
      scheduleStatus: scheduleFixture({ installState: 'installed' }),
      securityStatus: securityFixture({
        keyringStatus: {
          available: true,
          backend: 'macos-keychain',
          storedSecret: true,
        },
        warnings: [],
      }),
    })

    expect(
      screen.queryByText('Safari needs Full Disk Access'),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByText('System keychain not available'),
    ).not.toBeInTheDocument()
    expect(screen.queryByText('Schedule needs review')).not.toBeInTheDocument()
  })
})
