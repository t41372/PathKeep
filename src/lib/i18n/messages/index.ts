/**
 * Assembles the per-feature catalogs. Each feature owns one file with all
 * three languages side by side, so a copy change is reviewed in one place.
 */
import type { MessagePath, ResolvedLanguage } from '../define'
import { ask } from './ask'
import { backup } from './backup'
import { backupImport } from './backup-import'
import { backupRuns } from './backup-runs'
import { backupSchedule } from './backup-schedule'
import { common } from './common'
import { history } from './history'
import { home } from './home'
import { insights } from './insights'
import { onboarding } from './onboarding'
import { settings } from './settings'
import { settingsAbout } from './settings-about'
import { settingsAi } from './settings-ai'
import { settingsBackground } from './settings-background'
import { settingsStorage } from './settings-storage'
import { shell } from './shell'

const namespaces = {
  common,
  shell,
  home,
  history,
  insights,
  ask,
  backup,
  backupSchedule,
  backupImport,
  backupRuns,
  settings,
  settingsAi,
  settingsBackground,
  settingsStorage,
  settingsAbout,
  onboarding,
}

type Namespaces = typeof namespaces
type Catalog = { [N in keyof Namespaces]: Namespaces[N]['en'] }

function pick(lang: ResolvedLanguage) {
  return Object.fromEntries(
    Object.entries(namespaces).map(([name, ns]) => [name, ns[lang]]),
  ) as unknown as Catalog
}

export const messages: Record<ResolvedLanguage, Catalog> = {
  en: pick('en'),
  'zh-CN': pick('zh-CN'),
  'zh-TW': pick('zh-TW'),
}

export type MessageKey = MessagePath<Catalog>
