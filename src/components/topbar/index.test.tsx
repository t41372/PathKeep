import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test } from 'vitest'
import { onboardingScreen } from '../../app/router'
import { ShellDataProvider } from '../../app/shell-data'
import { I18nProvider } from '../../lib/i18n'
import { Topbar } from './index'

describe('Topbar', () => {
  test('renders the active screen metadata and shell actions', async () => {
    render(
      <I18nProvider>
        <ShellDataProvider>
          <MemoryRouter>
            <Topbar
              screen={{
                ...onboardingScreen,
                title: 'Dashboard',
                subtitle: 'Archive overview & system status',
                labelKey: undefined,
                titleKey: undefined,
                subtitleKey: undefined,
              }}
            />
          </MemoryRouter>
        </ShellDataProvider>
      </I18nProvider>,
    )

    expect(screen.getByRole('heading', { name: 'Dashboard' })).toBeVisible()
    expect(
      screen.getByRole('searchbox', { name: 'Search history' }),
    ).toBeVisible()
    expect(
      await screen.findByRole('button', { name: /Initialize first/ }),
    ).toBeVisible()
  })
})
