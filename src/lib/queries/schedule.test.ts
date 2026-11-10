/**
 * Failure modes: cached estimates move to an epoch grid or a backup interval;
 * an estimate exactly at now stays in the past; missing evidence invents a time.
 * These clock boundaries are impractical to hold fixed in the desktop E2E.
 */
import { describe, expect, it } from 'vitest'
import type { ScheduleStatus } from '@/lib/types'
import { nextScheduledBackup } from './schedule'

const status: ScheduleStatus = {
  platform: 'macos',
  label: 'test.pathkeep.backup',
  dueAfterHours: 24,
  checkIntervalHours: 6,
  applySupported: true,
  installState: 'installed',
  detectedFiles: [],
  manualSteps: [],
  warnings: [],
  lastSuccessfulBackupAt: '2026-11-08T09:40:00Z',
  nextScheduledBackupAt: '2026-11-09T06:17:00Z',
  wakeIntervalSeconds: 3_600,
}

describe('nextScheduledBackup', () => {
  it.each([
    ['2026-11-09T06:00:00Z', '2026-11-09T06:17:00Z'],
    ['2026-11-09T06:17:00Z', '2026-11-09T07:17:00Z'],
    ['2026-11-09T06:18:00Z', '2026-11-09T07:17:00Z'],
    ['2026-11-09T10:20:00Z', '2026-11-09T11:17:00Z'],
  ])('preserves the backend wake grid at %s', (now, expected) => {
    expect(nextScheduledBackup(status, Date.parse(now))?.toISOString()).toBe(
      new Date(expected).toISOString(),
    )
  })

  it('does not invent a time without an installed schedule and valid clock evidence', () => {
    const now = Date.parse('2026-11-09T10:20:00Z')
    for (const value of [
      undefined,
      { ...status, installState: 'not-installed' },
      { ...status, nextScheduledBackupAt: null },
      { ...status, nextScheduledBackupAt: 'invalid' },
      { ...status, wakeIntervalSeconds: null },
      { ...status, wakeIntervalSeconds: 0 },
      { ...status, wakeIntervalSeconds: NaN },
    ]) {
      expect(nextScheduledBackup(value, now)).toBeNull()
    }
  })
})
