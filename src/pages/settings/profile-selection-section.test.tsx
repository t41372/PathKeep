/**
 * @file profile-selection-section.test.tsx
 * @description Focused coverage for Settings browser-profile selection rows.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Verify selected profile rows, browser labels, history detection text, and disabled saving state.
 * - Protect the route-owned toggle callback contract.
 *
 * ## Not responsible for
 * - Re-testing browser discovery or config persistence.
 * - Re-testing shared browser icon asset rendering.
 *
 * ## Dependencies
 * - Uses the shipped i18n provider and production BrowserProfile shape.
 *
 * ## Performance notes
 * - Pure render test; no backend calls or filesystem access.
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import type { BrowserProfile } from '../../lib/types'
import { ProfileSelectionSection } from './profile-selection-section'
import type { SettingsSectionNavItem } from './section-nav-items'

const navItem: SettingsSectionNavItem = {
  key: 'profiles',
  id: 'profiles',
  label: 'Browser profiles',
  icon: 'language',
}

describe('ProfileSelectionSection', () => {
  test('renders selected and unavailable profile rows and forwards toggles', async () => {
    const user = userEvent.setup()
    const onToggleProfile = vi.fn().mockResolvedValue(undefined)

    render(
      <I18nProvider>
        <ProfileSelectionSection
          navItem={navItem}
          state={{
            profiles: [
              createProfile({
                browserName: 'Google Chrome',
                profileId: 'chrome:Default',
                profileName: 'Personal research',
                browserVersion: '124.0.0',
                historyExists: true,
                historyFileName: '',
              }),
              createProfile({
                browserName: 'Safari',
                profileId: 'safari:Work',
                profileName: 'Work',
                browserVersion: null,
                historyExists: false,
                historyFileName: '',
              }),
            ],
            saving: false,
            selectedIds: new Set(['chrome:Default']),
            onToggleProfile,
          }}
        />
      </I18nProvider>,
    )

    expect(screen.getByText('Browser profiles')).toBeVisible()
    expect(screen.getByText('Google Chrome / Personal research')).toBeVisible()
    expect(screen.getByText('Safari / Work')).toBeVisible()
    expect(screen.getByText('History')).toBeVisible()
    expect(
      screen.queryByText('/Users/test/chrome:Default'),
    ).not.toBeInTheDocument()
    expect(screen.getByText('History found · 124.0.0')).toBeVisible()
    expect(screen.getByText('No history file found')).toBeVisible()

    await user.click(
      screen.getByRole('button', { name: new RegExp('Safari / Work') }),
    )
    expect(onToggleProfile).toHaveBeenCalledWith('safari:Work')
  })

  test('disables profile rows while settings are saving', () => {
    render(
      <I18nProvider>
        <ProfileSelectionSection
          navItem={navItem}
          state={{
            profiles: [
              createProfile({
                browserName: 'Google Chrome',
                profileId: 'chrome:Default',
                profileName: 'Default',
                browserVersion: null,
                historyExists: true,
              }),
            ],
            saving: true,
            selectedIds: new Set(),
            onToggleProfile: vi.fn().mockResolvedValue(undefined),
          }}
        />
      </I18nProvider>,
    )

    expect(
      screen.getByRole('button', {
        name: new RegExp('Google Chrome / Default'),
      }),
    ).toBeDisabled()
  })
})

function createProfile(
  profile: Pick<
    BrowserProfile,
    | 'browserName'
    | 'browserVersion'
    | 'historyExists'
    | 'profileId'
    | 'profileName'
  > &
    Partial<Pick<BrowserProfile, 'historyFileName'>>,
): BrowserProfile {
  return {
    ...profile,
    browserFamily: profile.browserName === 'Safari' ? 'safari' : 'chromium',
    userName: null,
    profilePath: `/Users/test/${profile.profileId}`,
    historyPath: profile.historyExists
      ? `/Users/test/${profile.profileId}/History`
      : null,
    faviconsPath: null,
    historyReadable: profile.historyExists,
    accessIssue: null,
    historyFileName:
      profile.historyFileName ??
      (profile.browserName === 'Safari' ? 'History.db' : 'History'),
    historyBytes: profile.historyExists ? 1024 : 0,
    faviconsBytes: 0,
    supportingBytes: profile.historyExists ? 1024 : 0,
    retentionBoundary: {
      kind:
        profile.browserName === 'Safari' ? 'macos-safari' : 'browser-managed',
      localDays: profile.browserName === 'Safari' ? 365 : 90,
    },
  }
}
