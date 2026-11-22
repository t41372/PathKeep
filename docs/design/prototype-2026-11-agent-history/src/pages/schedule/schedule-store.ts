/**
 * The prototype's stand-in for `useScheduleStatus` and the saved interval
 * (`config.dueAfterHours`): one fixture the switcher picks and the dialog's
 * install / turn off / remove-old-job change, so a confirmed change shows on
 * the card the way a refetch would.
 */
import { useSyncExternalStore } from 'react'
import { NOW } from '~/data/fixture'
import { auditPath, statusFor, type AttemptsCase, type InstallState, type PermissionCause, type Platform, type ScheduleFixture } from './schedule-data'

let fixture: ScheduleFixture = { platform: 'macos', state: 'installed', cause: 'not-loaded', attempts: 'normal', hours: 6, checkedAt: NOW - 2 * 60_000 }
let status = statusFor(fixture)
const listeners = new Set<() => void>()

function set(next: Partial<ScheduleFixture>) {
  fixture = { ...fixture, ...next }
  status = statusFor(fixture)
  listeners.forEach((l) => l())
}

const subscribe = (l: () => void) => {
  listeners.add(l)
  return () => listeners.delete(l)
}

export function useSchedule() {
  return useSyncExternalStore(subscribe, () => fixture)
}

export function useScheduleStatus() {
  return useSyncExternalStore(subscribe, () => status)
}

/** Prototype switcher: show another state. Clears what a dialog changed. */
export function pickFixture(next: Partial<Pick<ScheduleFixture, 'platform' | 'state' | 'cause' | 'attempts' | 'hours'>>) {
  const platform = next.platform ?? fixture.platform
  let state: InstallState = next.state ?? fixture.state
  if (platform === 'linux') state = 'manual-review'
  else if (state === 'manual-review') state = 'installed'
  if (platform === 'windows' && state === 'legacy-install-detected') state = 'installed'
  let cause: PermissionCause = next.cause ?? fixture.cause
  if (platform === 'windows' && !['access-denied', 'inspection-failed'].includes(cause)) cause = 'access-denied'
  if (platform === 'macos' && ['access-denied', 'inspection-failed'].includes(cause)) cause = 'not-loaded'
  set({ ...next, platform, state, cause, lastAction: undefined })
}

/** Install, update or reinstall the plan that was shown, or (where PathKeep cannot install) only save the interval. */
export function applySchedule(hours: number) {
  if (fixture.platform === 'linux') return set({ hours })
  const files = status.detectedFiles.filter((f) => !f.includes('dev.codex'))
  set({
    hours,
    state: fixture.state === 'legacy-install-detected' ? 'legacy-install-detected' : 'installed',
    lastAction: { action: 'apply', status: 'ok', message: '', at: new Date(NOW).toISOString(), auditPath: auditPath('apply', NOW), files: files.length ? files : [fixture.platform === 'windows' ? 'Task Scheduler:com.yi-ting.pathkeep.backup' : '/Users/mei/Library/LaunchAgents/com.yi-ting.pathkeep.backup.plist'] },
  })
}

export function removeSchedule() {
  set({ state: 'not-installed', lastAction: { action: 'remove', status: 'ok', message: '', at: new Date(NOW).toISOString(), auditPath: auditPath('remove', NOW), files: status.detectedFiles } })
}

export function repairLegacy() {
  set({ state: 'installed', lastAction: { action: 'repair', status: 'ok', message: '', at: new Date(NOW).toISOString(), auditPath: auditPath('repair', NOW), files: status.detectedFiles.filter((f) => f.includes('dev.codex')) } })
}

/** 「再檢查一次」: the status is read again. */
export function recheck() {
  set({ checkedAt: NOW })
}

export type { AttemptsCase, InstallState, PermissionCause, Platform }
