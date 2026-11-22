/**
 * Per-source numbers the Backup, Settings and Onboarding screens share:
 * harness folders with conversation counts and sizes, the invented browser
 * profiles, and the archive size of each data type.
 */
import arcIcon from '@/assets/browser-icons/arc.png'
import chromeIcon from '@/assets/browser-icons/chrome.png'
import firefoxIcon from '@/assets/browser-icons/firefox.svg'
import safariIcon from '@/assets/browser-icons/safari.png'
import { archive, harnesses, harnessOrder, totals } from '~/data/archive'
import { NOW } from '~/data/fixture'
import type { HarnessId } from '~/data/types'
import type { KindId } from '~/kinds'

export interface HarnessSource {
  id: HarnessId
  name: string
  root: string
  chats: number
  subagents: number
  /** Conversations the tool no longer has; only PathKeep's archive keeps them. */
  gone: number
  sourceBytes: number
  lastBackup: number
  /** A setting in the tool that deletes old conversations on its own. */
  autoDelete?: { setting: string; days: number; file: string; docs: string }
  /** Extra folders read for this tool besides `root`. */
  alsoReads?: string
}

export const harnessSources: HarnessSource[] = harnessOrder.map((id) => {
  const chats = archive.chats.filter((c) => c.harness === id)
  return {
    id,
    name: harnesses[id].name,
    root: harnesses[id].root,
    chats: chats.length,
    subagents: chats.reduce((s, c) => s + c.subagentCount, 0),
    gone: chats.filter((c) => c.source === 'gone').length,
    sourceBytes: chats.reduce((s, c) => s + c.sizeBytes, 0),
    lastBackup: NOW - 78 * 60_000 + (id === 'codex' ? 40_000 : 0),
    autoDelete:
      id === 'claude'
        ? {
            setting: 'cleanupPeriodDays',
            days: 30,
            file: '~/.claude/settings.json',
            docs: 'https://docs.claude.com/en/docs/claude-code/settings',
          }
        : undefined,
    alsoReads: id === 'codex' ? '~/.codex/archived_sessions' : undefined,
  }
})

export interface BrowserSource {
  name: string
  profile: string
  icon: string
  visits: number
  lastVisit: number
}

export const browserSources: BrowserSource[] = [
  { name: 'Chrome', profile: '個人', icon: chromeIcon, visits: 412_308, lastVisit: NOW - 6 * 60_000 },
  { name: 'Chrome', profile: '工作', icon: chromeIcon, visits: 96_115, lastVisit: NOW - 3 * 3600_000 },
  { name: 'Safari', profile: '預設', icon: safariIcon, visits: 188_902, lastVisit: NOW - 26 * 3600_000 },
  { name: 'Arc', profile: '預設', icon: arcIcon, visits: 51_447, lastVisit: NOW - 9 * 86_400_000 },
  { name: 'Firefox', profile: 'default-release', icon: firefoxIcon, visits: 7_310, lastVisit: NOW - 140 * 86_400_000 },
]

/** Agent archive is about 2.9× its source today (raw copy plus search index). */
export const AGENT_ARCHIVE_RATIO = 2.9

export const archiveBytes: Partial<Record<KindId, number>> = {
  browsing: 1_380_000_000,
  agent: Math.round(totals.sourceBytes * AGENT_ARCHIVE_RATIO),
}

export const sourceBytesOfKind: Partial<Record<KindId, number>> = {
  browsing: 2_150_000_000,
  agent: totals.sourceBytes,
}

/** How many records each type holds, as a person would count them. */
export const kindCounts: Partial<Record<KindId, string>> = {
  browsing: `${browserSources.reduce((s, b) => s + b.visits, 0).toLocaleString('zh-TW')} 筆造訪`,
  agent: `${totals.total.toLocaleString('zh-TW')} 段對話`,
}
