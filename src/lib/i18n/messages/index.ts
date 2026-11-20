/**
 * Assembles the per-feature catalogs. Each feature owns one file with all
 * three languages side by side, so a copy change is reviewed in one place.
 */
import type { MessagePath, ResolvedLanguage } from '../define'
import { agentBackup } from './agent-backup'
import { agentChats } from './agent-chats'
import { agentInfo } from './agent-info'
import { agentProjects } from './agent-projects'
import { agentReader } from './agent-reader'
import { agentRepair } from './agent-repair'
import { agentSearch } from './agent-search'
import { agentTasks } from './agent-tasks'
import { agentInsights } from './agent-insights'
import { agentInsightsCards } from './agent-insights-cards'
import { ask } from './ask'
import { backup } from './backup'
import { backupImport } from './backup-import'
import { backupRuns } from './backup-runs'
import { backupSchedule } from './backup-schedule'
import { common } from './common'
import { dataKinds } from './data-kinds'
import { enableAgent } from './enable-agent'
import { history } from './history'
import { historyPage } from './history-page'
import { historySearch } from './history-search'
import { home } from './home'
import { insights } from './insights'
import { maintenance } from './maintenance'
import { onboarding } from './onboarding'
import { settings } from './settings'
import { settingsAbout } from './settings-about'
import { settingsAgent } from './settings-agent'
import { settingsBrowsing } from './settings-browsing'
import { settingsAi } from './settings-ai'
import { settingsBackground } from './settings-background'
import { settingsStorage } from './settings-storage'
import { shell } from './shell'
import { spaces } from './spaces'
import { tasks } from './tasks'

const namespaces = {
  common,
  shell,
  spaces,
  tasks,
  dataKinds,
  enableAgent,
  agentBackup,
  agentChats,
  agentReader,
  agentInfo,
  agentProjects,
  agentRepair,
  agentSearch,
  agentTasks,
  agentInsights,
  agentInsightsCards,
  home,
  history,
  historyPage,
  historySearch,
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
  settingsAgent,
  settingsBrowsing,
  onboarding,
  maintenance,
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
