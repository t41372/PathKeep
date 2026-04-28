/**
 * This test file protects the front-end helper and contract logic in Onboarding Estimates.
 *
 * Why this file exists:
 * - Pure helpers are where we keep UI policy testable without booting the whole shell.
 * - When these tests fail, they usually point at a contract drift that would otherwise show up as subtle route regressions.
 *
 * Main declarations:
 * - This file is mostly internal implementation detail.
 *
 * Source-of-truth notes:
 * - Helper behavior should stay aligned with the same design, feature, and architecture docs that guide the UI surfaces consuming it.
 * - Prefer focused behavioral assertions over snapshotting implementation detail.
 */

import { describe, expect, test } from 'vitest'
import { estimateOnboardingStorage } from './onboarding-estimates'
import type { BrowserProfile } from './types'

const profiles: BrowserProfile[] = [
  {
    profileId: 'chrome:Default',
    profileName: 'Default',
    browserFamily: 'chromium',
    browserName: 'Chrome',
    profilePath: '/profiles/chrome-default',
    historyExists: true,
    historyFileName: 'History',
    historyBytes: 100,
    faviconsBytes: 10,
    supportingBytes: 5,
    retentionBoundary: {
      kind: 'browser-managed',
      localDays: null,
    },
  },
  {
    profileId: 'firefox:default-release',
    profileName: 'default-release',
    browserFamily: 'firefox',
    browserName: 'Firefox',
    profilePath: '/profiles/firefox-default',
    historyExists: true,
    historyFileName: 'places.sqlite',
    historyBytes: 60,
    faviconsBytes: 15,
    supportingBytes: 5,
    retentionBoundary: {
      kind: 'browser-managed',
      localDays: null,
    },
  },
  {
    profileId: 'safari:main',
    profileName: 'main',
    browserFamily: 'safari',
    browserName: 'Safari',
    profilePath: '/profiles/safari-main',
    historyExists: false,
    historyFileName: 'History.db',
    historyBytes: 500,
    faviconsBytes: 20,
    supportingBytes: 5,
    retentionBoundary: {
      kind: 'macos-safari',
      localDays: 365,
    },
  },
]

describe('onboarding storage estimates', () => {
  test('estimates storage from selected readable profiles only', () => {
    expect(
      estimateOnboardingStorage(profiles, [
        'chrome:Default',
        'firefox:default-release',
        'safari:main',
      ]),
    ).toEqual({
      profileCount: 2,
      sourceBytes: 195,
      archiveDbBytes: 156,
      manifestBytes: 262144,
      snapshotsBytes: 195,
      totalBytes: 262495,
    })
  })

  test('returns an empty estimate when nothing readable is selected', () => {
    expect(estimateOnboardingStorage(profiles, ['safari:main'])).toEqual({
      profileCount: 0,
      sourceBytes: 0,
      archiveDbBytes: 0,
      manifestBytes: 0,
      snapshotsBytes: 0,
      totalBytes: 0,
    })
  })

  test('scales manifest storage once readable profile count exceeds the base allowance', () => {
    const manyProfiles = Array.from({ length: 5 }, (_, index) => ({
      ...profiles[0],
      profileId: `chrome:Profile ${index + 1}`,
      profileName: `Profile ${index + 1}`,
    }))

    expect(
      estimateOnboardingStorage(
        manyProfiles,
        manyProfiles.map((profile) => profile.profileId),
      ),
    ).toEqual({
      profileCount: 5,
      sourceBytes: 575,
      archiveDbBytes: 460,
      manifestBytes: 327680,
      snapshotsBytes: 575,
      totalBytes: 328715,
    })
  })
})
