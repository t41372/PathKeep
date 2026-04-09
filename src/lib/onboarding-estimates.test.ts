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
})
