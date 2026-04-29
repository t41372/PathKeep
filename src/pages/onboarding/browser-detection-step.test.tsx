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

import { render, screen } from '@testing-library/react'
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
          onToggleProfile={vi.fn()}
        />
      </I18nProvider>,
    )

    expect(
      screen.queryByText(onboardingT('firefoxSafariInfo')),
    ).not.toBeInTheDocument()
  })
})

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
