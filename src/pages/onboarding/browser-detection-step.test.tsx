/**
 * @file browser-detection-step.test.tsx
 * @description Focused render coverage for onboarding browser profile detection.
 * @module pages/onboarding
 *
 * ## Responsibilities
 * - Verify detected profiles render Firefox and unknown engine labels truthfully.
 * - Keep profile toggles and Full Disk Access recovery controls wired to route callbacks.
 *
 * ## Not responsible for
 * - Browser discovery IPC.
 * - Full onboarding route step orchestration.
 *
 * ## Dependencies
 * - Uses the real i18n provider and browser-retention helper copy.
 *
 * ## Performance notes
 * - Pure render tests keep onboarding profile-card coverage cheap and deterministic.
 */

import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, test, vi } from 'vitest'
import { I18nProvider, createNamespaceTranslator } from '../../lib/i18n'
import type { BrowserProfile } from '../../lib/types'
import { BrowserDetectionStep } from './browser-detection-step'

const commonT = createNamespaceTranslator('en', 'common')
const onboardingT = createNamespaceTranslator('en', 'onboarding')

describe('BrowserDetectionStep', () => {
  test('renders Firefox and unknown engine profiles and wires recovery actions', async () => {
    const user = userEvent.setup()
    const onOpenFullDiskAccessSettings = vi.fn()
    const onToggleProfile = vi.fn()

    render(
      <I18nProvider>
        <BrowserDetectionStep
          browserProfiles={[
            browserProfileFixture({
              browserFamily: 'firefox',
              browserName: 'Firefox',
              profileId: 'firefox:default',
              profileName: 'default',
            }),
            browserProfileFixture({
              browserFamily: '',
              browserName: 'Unknown Browser',
              profileId: 'unknown:profile',
              profileName: 'Mystery',
            }),
          ]}
          busyAction={null}
          localError="Manual review required"
          selectedAccessIssueCount={1}
          selectedCount={1}
          selectedProfileIds={['firefox:default']}
          onBack={vi.fn()}
          onContinue={vi.fn()}
          onOpenFullDiskAccessSettings={onOpenFullDiskAccessSettings}
          onRecheck={vi.fn()}
          onToggleProfile={onToggleProfile}
        />
      </I18nProvider>,
    )

    expect(screen.getAllByText(/Firefox/).length).toBeGreaterThan(0)
    expect(screen.getByText(/unknown/)).toBeVisible()
    expect(screen.getByRole('alert')).toHaveTextContent(
      'Manual review required',
    )

    await user.click(screen.getByLabelText('Firefox / default'))
    expect(onToggleProfile).toHaveBeenCalledWith('firefox:default')

    await user.click(
      screen.getByRole('button', { name: 'Open Full Disk Access settings' }),
    )
    expect(onOpenFullDiskAccessSettings).toHaveBeenCalled()
  })

  test('orders readable profiles first and explains each browser access state', () => {
    const onContinue = vi.fn()

    render(
      <I18nProvider>
        <BrowserDetectionStep
          browserProfiles={[
            browserProfileFixture({
              browserFamily: 'safari',
              browserName: 'Safari',
              historyReadable: false,
              profileId: 'safari:Default',
              profileName: 'Default',
              retentionBoundary: {
                kind: 'macos-safari',
                localDays: null,
              },
            }),
            browserProfileFixture({
              browserFamily: 'chromium',
              browserName: 'Google Chrome',
              browserVersion: null,
              historyFileName: '',
              profileId: 'chrome:Default',
              profileName: 'Default',
            }),
            browserProfileFixture({
              browserFamily: 'chromium',
              browserName: 'Arc',
              historyReadable: false,
              profileId: 'arc:Default',
              profileName: 'Default',
            }),
            browserProfileFixture({
              browserFamily: 'safari',
              browserName: 'Safari',
              historyExists: false,
              historyReadable: false,
              profileId: 'safari:Missing',
              profileName: 'Missing',
              retentionBoundary: {
                kind: 'macos-safari',
                localDays: 90,
              },
            }),
            browserProfileFixture({
              browserFamily: 'brave',
              browserName: 'Brave',
              historyExists: false,
              historyFileName: '',
              historyPath: null,
              historyReadable: false,
              profileId: 'brave:Missing',
              profileName: 'Missing',
            }),
          ]}
          busyAction="Saving"
          localError={null}
          selectedAccessIssueCount={0}
          selectedCount={1}
          selectedProfileIds={['chrome:Default']}
          onBack={vi.fn()}
          onContinue={onContinue}
          onOpenFullDiskAccessSettings={vi.fn()}
          onRecheck={vi.fn()}
          onToggleProfile={vi.fn()}
        />
      </I18nProvider>,
    )

    expect(
      screen.getByText(
        onboardingT('browserEngineLabel', {
          engine: onboardingT('browserEngineChromium'),
          version: onboardingT('versionUnknown'),
        }),
      ),
    ).toBeVisible()
    expect(
      screen.getByText(commonT('browserRetentionManagedLabel')),
    ).toBeVisible()
    expect(
      screen.queryByText(
        '/Users/test/Library/Application Support/Browser/Profile',
      ),
    ).not.toBeInTheDocument()
    expect(screen.getAllByText('History').length).toBeGreaterThan(0)
    expect(screen.getAllByText(onboardingT('safariAccessHint')).length).toBe(2)
    expect(
      screen.getByText(onboardingT('browserProfileAccessHint')),
    ).toBeVisible()
    expect(
      screen.getByText(
        onboardingT('cannotReadHint').replace('{fileName}', 'Missing'),
      ),
    ).toBeVisible()
    expect(screen.getByText(onboardingT('firefoxSafariInfo'))).toBeVisible()
    expect(
      screen.getByRole('button', { name: onboardingT('continueButton') }),
    ).toBeDisabled()
    expect(onContinue).not.toHaveBeenCalled()
  })

  test('hides non-Chromium note when only Chromium profiles are detected', () => {
    render(
      <I18nProvider>
        <BrowserDetectionStep
          browserProfiles={[
            browserProfileFixture({
              browserFamily: 'chromium',
              browserName: 'Google Chrome',
              profileId: 'chrome:Default',
              profileName: 'Default',
            }),
          ]}
          busyAction={null}
          localError={null}
          selectedAccessIssueCount={0}
          selectedCount={1}
          selectedProfileIds={['chrome:Default']}
          onBack={vi.fn()}
          onContinue={vi.fn()}
          onOpenFullDiskAccessSettings={vi.fn()}
          onRecheck={vi.fn()}
          onToggleProfile={vi.fn()}
        />
      </I18nProvider>,
    )

    expect(
      screen.queryByText(onboardingT('firefoxSafariInfo')),
    ).not.toBeInTheDocument()
  })

  test('turns an empty list into actionable Full Disk Access guidance with a pending-aware re-check when access is denied', async () => {
    const user = userEvent.setup()
    const onOpenFullDiskAccessSettings = vi.fn()
    const recheckPending = deferred<void>()
    const onRecheck = vi.fn(() => recheckPending.promise)

    render(
      <I18nProvider>
        <BrowserDetectionStep
          browserDiscoveryIssue="macos-full-disk-access"
          browserProfiles={[]}
          busyAction={null}
          localError={null}
          selectedAccessIssueCount={0}
          selectedCount={0}
          selectedProfileIds={[]}
          onBack={vi.fn()}
          onContinue={vi.fn()}
          onOpenFullDiskAccessSettings={onOpenFullDiskAccessSettings}
          onRecheck={onRecheck}
          onToggleProfile={vi.fn()}
        />
      </I18nProvider>,
    )

    // The denial is explained — not disguised as a bland "0 browsers".
    expect(
      screen.getByText(onboardingT('fullDiskAccessEmptyTitle')),
    ).toBeVisible()
    expect(
      screen.getByText(onboardingT('fullDiskAccessEmptyBody')),
    ).toBeVisible()
    // No neutral empty copy when the real cause is a missing permission.
    expect(
      screen.queryByText(onboardingT('noBrowsersTitle')),
    ).not.toBeInTheDocument()

    await user.click(
      screen.getByRole('button', {
        name: onboardingT('openFullDiskAccessSettings'),
      }),
    )
    expect(onOpenFullDiskAccessSettings).toHaveBeenCalledTimes(1)

    await user.click(
      screen.getByRole('button', { name: onboardingT('recheckBrowsers') }),
    )
    expect(onRecheck).toHaveBeenCalledTimes(1)
    // Re-check surfaces a pending state (button disabled + pending label) rather
    // than freezing the step. With no aria-label, the button's accessible name
    // IS its visible text (WCAG 2.5.3), so it flips to the pending label.
    const pendingButton = screen.getByRole('button', {
      name: onboardingT('recheckingBrowsers'),
    })
    expect(pendingButton).toBeVisible()
    expect(pendingButton).toBeDisabled()

    recheckPending.resolve()
    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: onboardingT('recheckBrowsers') }),
      ).toBeEnabled(),
    )
  })

  test('surfaces an in-step error and re-enables the button when a re-check fails', async () => {
    // refreshAppData re-throws on a snapshot-fetch failure, and onboarding's
    // global error gate is suppressed while a snapshot still exists — so the
    // step must show the failure itself instead of silently flipping back.
    const user = userEvent.setup()
    const recheckPending = deferred<void>()
    const onRecheck = vi.fn(() => recheckPending.promise)

    render(
      <I18nProvider>
        <BrowserDetectionStep
          browserDiscoveryIssue="macos-full-disk-access"
          browserProfiles={[]}
          busyAction={null}
          localError={null}
          selectedAccessIssueCount={0}
          selectedCount={0}
          selectedProfileIds={[]}
          onBack={vi.fn()}
          onContinue={vi.fn()}
          onOpenFullDiskAccessSettings={vi.fn()}
          onRecheck={onRecheck}
          onToggleProfile={vi.fn()}
        />
      </I18nProvider>,
    )

    await user.click(
      screen.getByRole('button', { name: onboardingT('recheckBrowsers') }),
    )
    recheckPending.reject(new Error('snapshot fetch exploded'))

    // The failure is surfaced in-step (with the underlying detail) as an alert.
    const errorMessage = await screen.findByText(
      onboardingT('errorRecheckFailed', {
        detail: 'snapshot fetch exploded',
      }),
    )
    expect(errorMessage).toBeVisible()
    expect(errorMessage).toHaveAttribute('role', 'alert')
    // The button re-enables so the user can retry immediately.
    await waitFor(() =>
      expect(
        screen.getByRole('button', { name: onboardingT('recheckBrowsers') }),
      ).toBeEnabled(),
    )
  })

  test('shows the calm neutral empty message with no FDA nag when discovery succeeded and found nothing', () => {
    render(
      <I18nProvider>
        <BrowserDetectionStep
          browserProfiles={[]}
          busyAction={null}
          localError={null}
          selectedAccessIssueCount={0}
          selectedCount={0}
          selectedProfileIds={[]}
          onBack={vi.fn()}
          onContinue={vi.fn()}
          onOpenFullDiskAccessSettings={vi.fn()}
          onRecheck={vi.fn()}
          onToggleProfile={vi.fn()}
        />
      </I18nProvider>,
    )

    expect(screen.getByText(onboardingT('noBrowsersTitle'))).toBeVisible()
    expect(screen.getByText(onboardingT('noBrowsersBody'))).toBeVisible()
    // A genuinely empty machine must never see the permission nag.
    expect(
      screen.queryByText(onboardingT('fullDiskAccessEmptyTitle')),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByRole('button', {
        name: onboardingT('openFullDiskAccessSettings'),
      }),
    ).not.toBeInTheDocument()
    // A gentle re-check affordance is still offered.
    expect(
      screen.getByRole('button', { name: onboardingT('recheckBrowsers') }),
    ).toBeInTheDocument()
  })

  test('surfaces a detection-error callout for a discovery failure instead of the neutral or FDA empty state', () => {
    render(
      <I18nProvider>
        <BrowserDetectionStep
          browserDiscoveryIssue="discovery-error"
          browserProfiles={[]}
          busyAction={null}
          localError={null}
          selectedAccessIssueCount={0}
          selectedCount={0}
          selectedProfileIds={[]}
          onBack={vi.fn()}
          onContinue={vi.fn()}
          onOpenFullDiskAccessSettings={vi.fn()}
          onRecheck={vi.fn()}
          onToggleProfile={vi.fn()}
        />
      </I18nProvider>,
    )

    expect(screen.getByText(onboardingT('discoveryErrorTitle'))).toBeVisible()
    expect(screen.getByText(onboardingT('discoveryErrorBody'))).toBeVisible()
    expect(
      screen.queryByText(onboardingT('noBrowsersTitle')),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByText(onboardingT('fullDiskAccessEmptyTitle')),
    ).not.toBeInTheDocument()
  })
})

function deferred<T>() {
  let resolve: (value: T) => void = () => {}
  let reject: (error: unknown) => void = () => {}
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })

  return { promise, reject, resolve }
}

function browserProfileFixture(
  overrides: Partial<BrowserProfile> = {},
): BrowserProfile {
  return {
    profileId: 'chrome:Default',
    profileName: 'Default',
    browserFamily: 'chromium',
    browserName: 'Google Chrome',
    userName: null,
    profilePath: '/Users/test/Library/Application Support/Browser/Profile',
    historyPath:
      '/Users/test/Library/Application Support/Browser/Profile/History',
    faviconsPath: null,
    historyExists: true,
    historyReadable: true,
    accessIssue: null,
    browserVersion: '1.0',
    historyFileName: 'History',
    historyBytes: 1024,
    faviconsBytes: 0,
    supportingBytes: 0,
    retentionBoundary: {
      kind: 'browser-managed',
      localDays: null,
    },
    ...overrides,
  }
}
