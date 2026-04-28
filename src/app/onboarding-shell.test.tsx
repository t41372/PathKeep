/**
 * @file onboarding-shell.test.tsx
 * @description Focused coverage for the standalone onboarding shell wrapper.
 * @module app
 */

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createMemoryRouter, RouterProvider } from 'react-router-dom'
import { describe, expect, test } from 'vitest'
import { createNamespaceTranslator, I18nProvider } from '../lib/i18n'
import {
  ShellDataContext,
  type ShellDataContextValue,
} from './shell-data-context'
import { OnboardingShell } from './onboarding-shell'

const shellT = createNamespaceTranslator('en', 'shell')

describe('OnboardingShell', () => {
  test('renders route content and falls back to the busy action label', () => {
    renderOnboardingShell({
      busyAction: 'Preparing archive',
      busyOverlay: null,
    })

    expect(screen.getByTestId('onboarding-shell')).toBeVisible()
    expect(screen.getByText('Onboarding child')).toBeVisible()
    expect(screen.getByText(shellT('onboardingVersion'))).toBeVisible()
    expect(screen.getByText(shellT('onboardingLeaveHint'))).toBeVisible()
    expect(screen.getByText('Onboarding child').parentElement).toHaveStyle({
      position: 'relative',
      zIndex: '2',
    })
    expect(screen.getByText('Preparing archive')).toBeVisible()
  })

  test('prefers the structured busy overlay label when present', () => {
    renderOnboardingShell({
      busyAction: 'Preparing archive',
      busyOverlay: {
        label: 'Importing browser history',
        detail: 'Chrome Default',
        progressLabel: '2 of 3',
        progressValue: 0.66,
        steps: [],
        activeStep: undefined,
        logLines: [],
      },
    })

    expect(screen.getByText('Importing browser history')).toBeVisible()
  })

  test('lets users exit setup back to the main shell route', async () => {
    const user = userEvent.setup()
    renderOnboardingShell({})

    await user.click(
      screen.getByRole('button', {
        name: shellT('exitSetup'),
      }),
    )

    expect(await screen.findByText('Home target')).toBeVisible()
  })
})

function renderOnboardingShell(overrides: Partial<ShellDataContextValue>) {
  const router = createMemoryRouter(
    [
      {
        path: '/onboarding',
        element: (
          <I18nProvider>
            <ShellDataContext.Provider
              value={{ ...shellValue(), ...overrides } as ShellDataContextValue}
            >
              <OnboardingShell />
            </ShellDataContext.Provider>
          </I18nProvider>
        ),
        children: [
          {
            index: true,
            element: <p>Onboarding child</p>,
          },
        ],
      },
      {
        path: '/',
        element: <p>Home target</p>,
      },
    ],
    {
      initialEntries: ['/onboarding'],
    },
  )

  return render(<RouterProvider router={router} />)
}

function shellValue(): Partial<ShellDataContextValue> {
  return {
    busyAction: null,
    busyOverlay: null,
  }
}
