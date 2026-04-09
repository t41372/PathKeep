import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test } from 'vitest'
import { onboardingScreen } from '../../app/router'
import { ShellDataProvider } from '../../app/shell-data'
import { I18nProvider } from '../../lib/i18n'
import { ProfileScopeProvider } from '../../lib/profile-scope'
import { Topbar } from './index'

describe('Topbar', () => {
  test('renders the active screen metadata and shell actions', async () => {
    render(
      <I18nProvider>
        <ProfileScopeProvider>
          <ShellDataProvider>
            <MemoryRouter>
              <Topbar
                screen={{
                  ...onboardingScreen,
                  labelKey: 'navigation.dashboardLabel',
                  titleKey: 'navigation.dashboardTitle',
                  subtitleKey: 'navigation.dashboardSubtitle',
                }}
              />
            </MemoryRouter>
          </ShellDataProvider>
        </ProfileScopeProvider>
      </I18nProvider>,
    )

    expect(screen.getByRole('heading', { name: 'Dashboard' })).toBeVisible()
    expect(
      screen.getByRole('searchbox', { name: 'Search history' }),
    ).toBeVisible()
    expect(
      screen.getByRole('button', { name: 'Switch profile scope' }),
    ).toBeVisible()
    expect(screen.getByText('All profiles')).toBeVisible()
    expect(
      await screen.findByRole('button', { name: /Initialize first/ }),
    ).toBeVisible()
  })
})
