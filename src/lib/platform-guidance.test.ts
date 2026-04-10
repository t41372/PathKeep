import { describe, expect, test } from 'vitest'
import {
  hasSafariAccessIssue,
  keyringNeedsReview,
  needsSchedulerReview,
  normalizePlatform,
  platformLabelKey,
  platformSummaryKey,
} from './platform-guidance'
import type { BrowserProfile, ScheduleStatus, SecurityStatus } from './types'

const safariProfile: BrowserProfile = {
  profileId: 'safari:default',
  profileName: 'Safari',
  browserFamily: 'safari',
  browserName: 'Safari',
  userName: null,
  profilePath: '/Users/test/Library/Safari',
  historyPath: '/Users/test/Library/Safari/History.db',
  faviconsPath: null,
  historyExists: false,
  browserVersion: null,
  historyFileName: 'History.db',
  historyBytes: 18 * 1024 * 1024,
  faviconsBytes: 0,
  supportingBytes: 2 * 1024 * 1024,
  retentionBoundary: {
    kind: 'macos-safari',
    localDays: 365,
  },
}

const scheduleStatus: ScheduleStatus = {
  platform: 'macos',
  label: 'dev.codex.pathkeep.backup',
  dueAfterHours: 72,
  checkIntervalHours: 6,
  applySupported: true,
  installState: 'installed',
  detectedFiles: [],
  manualSteps: [],
  auditPath: null,
  lastSuccessfulBackupAt: null,
  warnings: [],
}

const securityStatus: SecurityStatus = {
  initialized: true,
  mode: 'encrypted',
  encrypted: true,
  unlocked: true,
  databasePath: '/tmp/pathkeep/archive.sqlite',
  keyringStatus: {
    available: true,
    backend: 'Test keyring',
    storedSecret: true,
  },
  strongholdPath: '/tmp/pathkeep/vault.hold',
  rememberDatabaseKeyInKeyring: true,
  lastSuccessfulBackupAt: null,
  warnings: [],
}

describe('platform guidance helpers', () => {
  test('normalizes platforms and resolves translated label keys', () => {
    expect(normalizePlatform()).toBe('macos')
    expect(normalizePlatform('windows')).toBe('windows')
    expect(normalizePlatform('linux')).toBe('linux')
    expect(normalizePlatform('plan9')).toBe('macos')

    expect(platformLabelKey('windows')).toBe('platform.windowsLabel')
    expect(platformLabelKey('linux')).toBe('platform.linuxLabel')
    expect(platformLabelKey('macos')).toBe('platform.macosLabel')

    expect(platformSummaryKey('windows')).toBe('platform.windowsSummary')
    expect(platformSummaryKey('linux')).toBe('platform.linuxSummary')
    expect(platformSummaryKey('macos')).toBe('platform.macosSummary')
  })

  test('surfaces Safari access, scheduler, and keyring review states', () => {
    expect(hasSafariAccessIssue([safariProfile])).toBe(true)
    expect(
      hasSafariAccessIssue([
        {
          ...safariProfile,
          historyExists: true,
        },
      ]),
    ).toBe(false)
    expect(
      hasSafariAccessIssue([
        {
          ...safariProfile,
          historyPath: null,
        },
      ]),
    ).toBe(false)
    expect(
      hasSafariAccessIssue([
        {
          ...safariProfile,
          profileId: 'chrome:Default',
          browserFamily: 'chromium',
          browserName: 'Chrome',
        },
      ]),
    ).toBe(false)

    expect(needsSchedulerReview(scheduleStatus)).toBe(false)
    expect(
      needsSchedulerReview({
        ...scheduleStatus,
        installState: 'manual-review',
      }),
    ).toBe(true)
    expect(
      needsSchedulerReview({
        ...scheduleStatus,
        installState: 'mismatch',
      }),
    ).toBe(true)
    expect(
      needsSchedulerReview({
        ...scheduleStatus,
        installState: 'permission-warning',
      }),
    ).toBe(true)
    expect(
      needsSchedulerReview({
        ...scheduleStatus,
        installState: 'legacy-install-detected',
      }),
    ).toBe(true)
    expect(needsSchedulerReview(null)).toBe(false)

    expect(keyringNeedsReview(securityStatus)).toBe(false)
    expect(
      keyringNeedsReview({
        ...securityStatus,
        keyringStatus: {
          ...securityStatus.keyringStatus,
          available: false,
        },
      }),
    ).toBe(true)
    expect(
      keyringNeedsReview({
        ...securityStatus,
        warnings: ['Store the database key before auto-unlock.'],
      }),
    ).toBe(true)
    expect(keyringNeedsReview(null)).toBe(false)
  })
})
