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
    const onBack = vi.fn()
    const onContinue = vi.fn()
    const onSecurityCardClick = vi.fn()
    const onSelectArchiveMode = vi.fn()
    const onUpdateSecurityDraft = vi.fn()

    render(
      <I18nProvider>
        <SecurityStep
          archiveMode="Encrypted"
          busyAction={null}
          localError="something went wrong"
          onBack={onBack}
          onContinue={onContinue}
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

    // The localError prop should surface through the alert region so the
    // user can see why the previous attempt failed.
    expect(screen.getByRole('alert')).toHaveTextContent('something went wrong')

    const encryptedRadio = screen.getByRole('radio', {
      name: 'Use encryption',
    })
    await user.click(encryptedRadio)
    expect(onSelectArchiveMode).toHaveBeenCalledWith('Encrypted')

    // Typing in the password fields should forward partial draft updates
    // so the route owner can run cross-field validation before continue.
    await user.type(screen.getByPlaceholderText('Enter a password'), 'a')
    expect(onUpdateSecurityDraft).toHaveBeenCalledWith({ masterPassword: 'a' })
    await user.type(
      screen.getByPlaceholderText('Enter the same password again'),
      'b',
    )
    expect(onUpdateSecurityDraft).toHaveBeenCalledWith({ confirmPassword: 'b' })

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

    await user.click(screen.getByRole('button', { name: '← Back' }))
    expect(onBack).toHaveBeenCalledTimes(1)
    await user.click(screen.getByRole('button', { name: 'Continue →' }))
    expect(onContinue).toHaveBeenCalledTimes(1)
  })

  test('disables the radio triggers while another setup action is busy', () => {
    // When the route owner is already saving (`busyAction !== null`) we
    // must not allow another archive-mode flip mid-flight — the disabled
    // attribute is the only thing protecting the saveConfig sequence.
    render(
      <I18nProvider>
        <SecurityStep
          archiveMode="Plaintext"
          busyAction="initialize"
          localError={null}
          onBack={vi.fn()}
          onContinue={vi.fn()}
          onSecurityCardClick={vi.fn()}
          onSelectArchiveMode={vi.fn()}
          onUpdateSecurityDraft={vi.fn()}
          securityDraft={{
            confirmPassword: '',
            masterPassword: '',
            rememberKey: false,
          }}
        />
      </I18nProvider>,
    )

    expect(screen.getByRole('radio', { name: 'Use encryption' })).toBeDisabled()
    expect(
      screen.getByRole('radio', { name: 'Skip encryption' }),
    ).toBeDisabled()
    // Plaintext mode exposes the trade-off list rather than the password form.
    expect(screen.getByText(/No password needed/)).toBeInTheDocument()
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

  test('leaves the draft alone when the probe rejects but the user never opted into the keychain', async () => {
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
            rememberKey: false,
          }}
        />
      </I18nProvider>,
    )

    await waitFor(() => {
      expect(screen.getByRole('checkbox')).toBeDisabled()
    })
    // We never touched rememberKey: the draft was already false, so the
    // probe failure should not trigger a redundant onUpdateSecurityDraft.
    expect(onUpdateSecurityDraft).not.toHaveBeenCalled()
  })

  test('drops the success-branch update when the component unmounts before the probe resolves', async () => {
    // Reproduces the race where the user navigates away (or the step
    // unmounts) before keyring_status responds. The cancel-after-unmount
    // guard on line 55 must prevent the stale resolve from touching React
    // state and emitting onUpdateSecurityDraft.
    let resolveFn: (value: {
      available: boolean
      backend: string
      message: string | null
      storedSecret: boolean
    }) => void = () => {}
    vi.spyOn(backend, 'keyringStatus').mockReturnValue(
      new Promise((resolve) => {
        resolveFn = resolve
      }),
    )
    const onUpdateSecurityDraft = vi.fn()

    const { unmount } = render(
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

    unmount()
    resolveFn({
      available: false,
      backend: 'Linux Secret Service / keyutils',
      message: null,
      storedSecret: false,
    })
    // Let the microtask queue drain so the .then handler runs.
    await Promise.resolve()
    await Promise.resolve()

    expect(onUpdateSecurityDraft).not.toHaveBeenCalled()
  })

  test('drops the catch-branch update when the component unmounts before the probe rejects', async () => {
    // Same race as the resolve case but on the failure path: the catch
    // branch's `if (cancelled) return` on line 64 must short-circuit so we
    // do not call setState on an unmounted component or wipe rememberKey
    // for a user who has already navigated away.
    let rejectFn: (reason: unknown) => void = () => {}
    vi.spyOn(backend, 'keyringStatus').mockReturnValue(
      new Promise((_resolve, reject) => {
        rejectFn = reject
      }),
    )
    const onUpdateSecurityDraft = vi.fn()

    const { unmount } = render(
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

    unmount()
    rejectFn(new Error('bridge dropped after unmount'))
    await Promise.resolve()
    await Promise.resolve()

    expect(onUpdateSecurityDraft).not.toHaveBeenCalled()
  })
})
