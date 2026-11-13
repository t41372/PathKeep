/**
 * Data types PathKeep can keep. The shell (rail, tabs, Home, Backup,
 * Settings) is drawn from this list, so a new type is one entry here plus
 * its own pages; nothing else in the shell names a type.
 */
import {
  Bell,
  HardDriveDownload,
  House,
  Bot,
  ChartColumn,
  FolderGit2,
  Globe,
  History,
  MessageSquareText,
  MessagesSquare,
  SquareTerminal,
  Sparkles,
  type LucideIcon,
} from 'lucide-react'

export type KindId = 'browsing' | 'agent' | 'terminal' | 'messages' | 'webchat' | 'notifications'

export interface KindPage {
  id: string
  label: string
  icon: LucideIcon
  path: string
}

export interface Kind {
  id: KindId
  /** Full name, used in headings and settings. */
  label: string
  /** Rail label: four CJK characters at most. */
  short: string
  icon: LucideIcon
  /** One line on what gets lost without PathKeep. */
  why: string
  /** What the sources are called and which ones exist. */
  sources: string[]
  pages: KindPage[]
  /** Pages when each type is its own space (round 3): the type's pages plus its own backup. */
  spacePages: KindPage[]
  /** Hue of the space's tint and accent (oklch). Browsing keeps PathKeep's orange. */
  hue: number
  /** Built in this prototype; the others are placeholders that show the shell scaling. */
  real: boolean
}

export const kinds: Kind[] = [
  {
    id: 'browsing',
    label: '瀏覽紀錄',
    short: '瀏覽紀錄',
    icon: Globe,
    why: 'Chrome 只留 90 天，Safari 預設一年',
    sources: ['Chrome', 'Safari', 'Arc', 'Firefox'],
    pages: [
      { id: 'history', label: '紀錄', icon: History, path: '/browsing/history' },
      { id: 'insights', label: '洞察', icon: ChartColumn, path: '/browsing/insights' },
    ],
    spacePages: [
      { id: 'home', label: '首頁', icon: House, path: '/browsing/home' },
      { id: 'history', label: '紀錄', icon: History, path: '/browsing/history' },
      { id: 'insights', label: '洞察', icon: ChartColumn, path: '/browsing/insights' },
      { id: 'backup', label: '備份', icon: HardDriveDownload, path: '/browsing/backup' },
    ],
    hue: 48,
    real: true,
  },
  {
    id: 'agent',
    label: 'Agent 對話',
    short: 'Agent',
    icon: Bot,
    why: 'Claude Code 預設 30 天清掉舊對話，Codex 會被手動清理',
    sources: ['Claude Code', 'Codex', 'Pi', 'DeepSeek Harness'],
    pages: [
      { id: 'chats', label: '紀錄', icon: MessagesSquare, path: '/agent/chats' },
      { id: 'projects', label: '專案', icon: FolderGit2, path: '/agent/projects' },
      { id: 'insights', label: '洞察', icon: ChartColumn, path: '/agent/insights' },
    ],
    spacePages: [
      { id: 'insights', label: '洞察', icon: ChartColumn, path: '/agent/insights' },
      { id: 'chats', label: '紀錄', icon: MessagesSquare, path: '/agent/chats' },
      { id: 'projects', label: '專案', icon: FolderGit2, path: '/agent/projects' },
      { id: 'backup', label: '備份', icon: HardDriveDownload, path: '/agent/backup' },
    ],
    hue: 195,
    real: true,
  },
  {
    id: 'terminal',
    label: '終端機指令',
    short: '終端機',
    icon: SquareTerminal,
    why: 'shell 只保留最後 1 萬行，多個視窗會互相覆蓋',
    sources: ['zsh', 'bash', 'fish', 'Atuin'],
    pages: [{ id: 'items', label: '指令', icon: SquareTerminal, path: '/k/terminal' }],
    spacePages: [],
    hue: 150,
    real: false,
  },
  {
    id: 'messages',
    label: '聊天訊息',
    short: '聊天',
    icon: MessageSquareText,
    why: '免費版 Slack 只看得到 90 天',
    sources: ['Slack', 'Discord', 'Telegram'],
    pages: [{ id: 'items', label: '訊息', icon: MessageSquareText, path: '/k/messages' }],
    spacePages: [],
    hue: 300,
    real: false,
  },
  {
    id: 'webchat',
    label: 'AI 網頁對話',
    short: 'AI 網頁',
    icon: Sparkles,
    why: '帳號被停用或刪除對話就拿不回來',
    sources: ['ChatGPT 匯出檔', 'Claude.ai 匯出檔', 'Gemini 匯出檔'],
    pages: [{ id: 'items', label: '對話', icon: Sparkles, path: '/k/webchat' }],
    spacePages: [],
    hue: 255,
    real: false,
  },
  {
    id: 'notifications',
    label: '系統通知',
    short: '通知',
    icon: Bell,
    why: 'macOS 通知中心過幾天就清空',
    sources: ['macOS 通知中心'],
    pages: [{ id: 'items', label: '通知', icon: Bell, path: '/k/notifications' }],
    spacePages: [],
    hue: 15,
    real: false,
  },
]

for (const k of kinds) if (k.spacePages.length === 0) k.spacePages = k.pages

export const kindById = Object.fromEntries(kinds.map((k) => [k.id, k])) as Record<KindId, Kind>

/** A page's tab or rail item is lit for its own path and the pages under it (a chat opened full page counts as 對話). */
export function pageActive(pagePath: string, pathname: string) {
  return pathname === pagePath || pathname.startsWith(pagePath + '/') || (pagePath === '/agent/chats' && pathname.startsWith('/agent/chat/'))
}

/** Which space a path belongs to (none for shared pages: Ask, Settings, onboarding). */
export function spaceOfPath(pathname: string): KindId | undefined {
  if (pathname.startsWith('/browsing/')) return 'browsing'
  if (pathname.startsWith('/agent/')) return 'agent'
  const m = /^\/k\/([a-z]+)/.exec(pathname)
  return m ? (m[1] as KindId) : undefined
}
