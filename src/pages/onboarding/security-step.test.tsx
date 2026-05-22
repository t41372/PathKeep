/**
 * @file security-step.test.tsx
 * @description Presentational coverage for onboarding archive security controls.
 * @module pages/onboarding
 *
 * ## Responsibilities
 * - Verify encrypted-form field changes are forwarded to the route-owned draft.
 * - Keep card and radio handlers covered without re-running the full onboarding flow.
 *
 * ## Not responsible for
 * - Re-testing archive initialization or keyring persistence.
 * - Re-testing validation in the route owner.
 *
 * ## Dependencies
 * - Uses the shipped i18n provider because the component reads onboarding copy directly.
 *
 * ## Performance notes
 * - Pure render and click/change coverage only.
 */

import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, test, vi } from 'vitest'
import { backend } from '../../lib/backend'
import { I18nProvider } from '../../lib/i18n'
import { SecurityStep } from './security-step'

describe('SecurityStep', () => {
  afterEach(() => {
    vi.restoreAllMocks()
  })

  test('forwards encrypted draft field changes and card selections', async () => {
    const user = userEvent.setup()
    const onSecurityCardClick = vi.fn()
    const onSelectArchiveMode = vi.fn()
    const onUpdateSecurityDraft = vi.fn()

    render(
      <I18nProvider>
        <SecurityStep
          archiveMode="Encrypted"
          busyAction={null}
          localError={null}
          onBack={vi.fn()}
          onContinue={vi.fn()}
          onSecurityCardClick={onSecurityCardClick}
          onSelectArchiveMode={onSelectArchiveMode}
          onUpdateSecurityDraft={onUpdateSecurityDraft}
          securityDraft={{
            confirmPassword: '',
            masterPassword: '',
            rememberKey: false,
          }}
        />
      </I18nProvider>,
    )

    const plaintextRadio = screen.getByRole('radio', {
      name: 'Skip encryption',
    })
    await user.click(plaintextRadio)
    expect(onSelectArchiveMode).toHaveBeenCalledWith('Plaintext')

    const plaintextCard = plaintextRadio.closest('.security-option')
    expect(plaintextCard).toBeInstanceOf(HTMLElement)
    await user.click(plaintextCard as HTMLElement)
    expect(onSecurityCardClick).toHaveBeenCalledWith(
      'Plaintext',
      expect.any(EventTarget),
    )

    await user.click(screen.getByRole('checkbox'))
    expect(onUpdateSecurityDraft).toHaveBeenCalledWith({ rememberKey: true })
  })

  test('disables remember-in-keychain when the probe reports no backend', async () => {
    vi.spyOn(backend, 'keyringStatus').mockResolvedValue({
      available: false,
      backend: 'Linux Secret Service / keyutils',
      storedSecret: false,
      message: 'A native keyring backend is not available on this machine.',
    })
    const onUpdateSecurityDraft = vi.fn()

    render(
      <I18nProvider>
        <SecurityStep
          archiveMode="Encrypted"
          busyAction={null}
          localError={null}
          onBack={vi.fn()}
          onContinue={vi.fn()}
          onSecurityCardClick={vi.fn()}
          onSelectArchiveMode={vi.fn()}
          onUpdateSecurityDraft={onUpdateSecurityDraft}
          securityDraft={{
            confirmPassword: '',
            masterPassword: '',
            rememberKey: true,
          }}
        />
      </I18nProvider>,
    )

    await waitFor(() => {
      expect(screen.getByRole('checkbox')).toBeDisabled()
    })
    expect(
      screen.getByTestId('onboarding-keyring-unavailable'),
    ).toBeInTheDocument()
    // Stale rememberKey draft must be cleared so the create-archive call
    // does not try to write to a backend we know is unavailable.
    expect(onUpdateSecurityDraft).toHaveBeenCalledWith({ rememberKey: false })
  })

  test('treats a probe rejection as no keyring backend', async () => {
    vi.spyOn(backend, 'keyringStatus').mockRejectedValue(
      new Error('bridge offline'),
    )
    const onUpdateSecurityDraft = vi.fn()

    render(
      <I18nProvider>
        <SecurityStep
          archiveMode="Encrypted"
          busyAction={null}
          localError={null}
          onBack={vi.fn()}
          onContinue={vi.fn()}
          onSecurityCardClick={vi.fn()}
          onSelectArchiveMode={vi.fn()}
          onUpdateSecurityDraft={onUpdateSecurityDraft}
          securityDraft={{
            confirmPassword: '',
            masterPassword: '',
            rememberKey: true,
          }}
        />
      </I18nProvider>,
    )

    await waitFor(() => {
      expect(screen.getByRole('checkbox')).toBeDisabled()
    })
    expect(onUpdateSecurityDraft).toHaveBeenCalledWith({ rememberKey: false })
  })
})
