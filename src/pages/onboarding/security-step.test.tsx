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

import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import { SecurityStep } from './security-step'

describe('SecurityStep', () => {
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
})
