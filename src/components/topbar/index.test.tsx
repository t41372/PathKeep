import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, test } from 'vitest'
import { onboardingScreen } from '../../app/router'
import { ShellDataProvider } from '../../app/shell-data'
import { Topbar } from './index'

describe('Topbar', () => {
  test('renders the active screen metadata and shell actions', async () => {
    render(
      <ShellDataProvider>
        <MemoryRouter>
          <Topbar
            screen={{
              ...onboardingScreen,
              title: 'Dashboard',
              subtitle: 'Archive overview & system status',
            }}
          />
        </MemoryRouter>
      </ShellDataProvider>,
    )

    expect(screen.getByRole('heading', { name: 'Dashboard' })).toBeVisible()
    expect(
      screen.getByRole('searchbox', { name: 'Search history' }),
    ).toBeVisible()
    expect(
      screen.getByRole('link', { name: 'Review onboarding' }),
    ).toBeVisible()
    expect(
      await screen.findByRole('button', { name: 'Initialize first' }),
    ).toBeVisible()
  })
})
