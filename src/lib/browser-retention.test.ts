import { describe, expect, test } from 'vitest'
import { browserRetentionMeta } from './browser-retention'

const t = (key: string, values?: Record<string, number | string>) =>
  values?.days ? `${key}:${values.days}` : key

describe('browserRetentionMeta', () => {
  test('treats Safari as a documented local-history window', () => {
    expect(
      browserRetentionMeta(
        {
          profileId: 'safari:default',
          profileName: 'Default',
          browserFamily: 'safari',
          browserName: 'Safari',
          userName: null,
          profilePath: '~/Library/Safari',
          historyPath: '~/Library/Safari/History.db',
          faviconsPath: null,
          historyExists: true,
          browserVersion: '18.4',
          historyFileName: 'History.db',
          historyBytes: 10,
          faviconsBytes: 0,
          supportingBytes: 0,
          retentionBoundary: { kind: 'macos-safari', localDays: 365 },
        },
        t,
      ),
    ).toEqual({
      label: 'browserRetentionSafariLabel:365',
      body: 'browserRetentionSafariBody',
    })
  })

  test('treats Chromium and Firefox as browser-managed retention', () => {
    expect(
      browserRetentionMeta(
        {
          profileId: 'chrome:Default',
          profileName: 'Default',
          browserFamily: 'chromium',
          browserName: 'Chrome',
          userName: null,
          profilePath: '~/Library/Application Support/Google/Chrome/Default',
          historyPath:
            '~/Library/Application Support/Google/Chrome/Default/History',
          faviconsPath:
            '~/Library/Application Support/Google/Chrome/Default/Favicons',
          historyExists: true,
          browserVersion: '146',
          historyFileName: 'History',
          historyBytes: 10,
          faviconsBytes: 1,
          supportingBytes: 1,
          retentionBoundary: { kind: 'browser-managed', localDays: null },
        },
        t,
      ),
    ).toEqual({
      label: 'browserRetentionManagedLabel',
      body: 'browserRetentionManagedBody',
    })
  })
})
