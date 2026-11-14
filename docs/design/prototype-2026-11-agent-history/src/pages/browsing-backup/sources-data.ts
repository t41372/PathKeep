/**
 * Invented browser profiles for the browsing Backup page, shaped like the
 * snapshot's `BrowserProfile` and `load_source_stats`'s `SourceStats`. Chosen
 * so every branch of the Sources card shows: a browser with one profile, one
 * with three (open), one with four and no problem (closed), Safari without
 * Full Disk Access, a profile switched off, a profile with no visits, and two
 * profiles whose history file is gone.
 */
import { NOW } from '~/data/fixture'

export interface Profile {
  profileId: string
  browserName: string
  browserFamily: 'chromium' | 'firefox' | 'safari'
  profileName: string
  historyExists: boolean
  /** False when the file is there but PathKeep may not read it. */
  historyReadable?: boolean
  accessIssue?: string
}

export interface Stat {
  profileId: string
  visitCount: number
  lastVisitAt: number | null
}

const HOUR = 3600_000
const DAY = 24 * HOUR

export const profiles: Profile[] = [
  { profileId: 'chrome:Default', browserName: 'Google Chrome', browserFamily: 'chromium', profileName: '個人', historyExists: true },
  { profileId: 'chrome:Profile 1', browserName: 'Google Chrome', browserFamily: 'chromium', profileName: '工作', historyExists: true },
  { profileId: 'chrome:Profile 2', browserName: 'Google Chrome', browserFamily: 'chromium', profileName: '學校', historyExists: true },
  {
    profileId: 'safari:default',
    browserName: 'Safari',
    browserFamily: 'safari',
    profileName: 'Default',
    historyExists: true,
    historyReadable: false,
    accessIssue: 'Operation not permitted (os error 1): ~/Library/Safari/History.db',
  },
  { profileId: 'arc:Default', browserName: 'Arc', browserFamily: 'chromium', profileName: 'Default', historyExists: true },
  { profileId: 'edge:Default', browserName: 'Microsoft Edge', browserFamily: 'chromium', profileName: 'Default', historyExists: true },
  { profileId: 'edge:Profile 1', browserName: 'Microsoft Edge', browserFamily: 'chromium', profileName: 'Profile 1', historyExists: true },
  { profileId: 'edge:Profile 2', browserName: 'Microsoft Edge', browserFamily: 'chromium', profileName: 'Profile 2', historyExists: true },
  { profileId: 'edge:Profile 3', browserName: 'Microsoft Edge', browserFamily: 'chromium', profileName: 'Profile 3', historyExists: true },
  { profileId: 'firefox:default-release', browserName: 'Firefox', browserFamily: 'firefox', profileName: 'default-release', historyExists: true },
  { profileId: 'brave:Default', browserName: 'Brave', browserFamily: 'chromium', profileName: 'Default', historyExists: false },
  { profileId: 'opera:Default', browserName: 'Opera', browserFamily: 'chromium', profileName: 'Default', historyExists: false },
]

/** Firefox is switched off (已暫停). */
export const initiallySelected = profiles.map((p) => p.profileId).filter((id) => id !== 'firefox:default-release')

export const stats: Stat[] = [
  { profileId: 'chrome:Default', visitCount: 412_308, lastVisitAt: NOW - 6 * 60_000 },
  { profileId: 'chrome:Profile 1', visitCount: 96_115, lastVisitAt: NOW - 3 * HOUR },
  { profileId: 'chrome:Profile 2', visitCount: 8_904, lastVisitAt: NOW - 12 * DAY },
  { profileId: 'safari:default', visitCount: 188_902, lastVisitAt: NOW - 26 * HOUR },
  { profileId: 'arc:Default', visitCount: 51_447, lastVisitAt: NOW - 9 * DAY },
  { profileId: 'edge:Default', visitCount: 23_016, lastVisitAt: NOW - 2 * DAY },
  { profileId: 'edge:Profile 1', visitCount: 4_210, lastVisitAt: NOW - 40 * DAY },
  { profileId: 'edge:Profile 2', visitCount: 1_302, lastVisitAt: NOW - 95 * DAY },
  // Backed up but never used: 「還沒有瀏覽記錄」.
  { profileId: 'edge:Profile 3', visitCount: 0, lastVisitAt: null },
  { profileId: 'firefox:default-release', visitCount: 7_310, lastVisitAt: NOW - 140 * DAY },
]

/** `{profile}` in a run's sentences, as the Sources card writes it. */
export function profileName(profileId: string | null | undefined) {
  if (!profileId) return ''
  const profile = profiles.find((p) => p.profileId === profileId)
  return profile ? `${profile.browserName} ${profile.profileName}` : profileId
}
