/**
 * What the onboarding sources step lists for each chosen type: today's two
 * types as the app would find them, and a stress set (many profiles, extra
 * tool folders, the planned types, two made-up ones) for judging the layout
 * when types and sources multiply. The stress set is prototype-only.
 */
import { Clipboard, Download, type LucideIcon } from 'lucide-react'
import type { ReactNode } from 'react'
import arcIcon from '@/assets/browser-icons/arc.png'
import braveIcon from '@/assets/browser-icons/brave.svg'
import chromeIcon from '@/assets/browser-icons/chrome.png'
import edgeIcon from '@/assets/browser-icons/edge.png'
import firefoxIcon from '@/assets/browser-icons/firefox.svg'
import operaIcon from '@/assets/browser-icons/opera.png'
import safariIcon from '@/assets/browser-icons/safari.png'
import vivaldiIcon from '@/assets/browser-icons/vivaldi.png'
import { HarnessBadge } from '~/components/bits'
import { bytes, num } from '~/data/format'
import { kindById, kinds, type KindId } from '~/kinds'
import { harnessSources } from '../backup/sources-data'

export interface StepSource {
  id: string
  /** Sources with the same group sit under one heading when the list is long. */
  group: string
  icon: ReactNode
  name: string
  /** Second line: a profile name, or the folder read (set `mono`). */
  detail: string
  mono?: boolean
  /** Right side, first line and second line. */
  amount: string
  /** What the first line measures, shown on hover (a profile's is its history file). */
  amountTitle?: string
  extra?: string
  /** The system will not let PathKeep read it until the user grants access. */
  blocked?: boolean
}

export interface StepKind {
  /** A registry type, or a made-up one in the stress set. */
  id: string
  label: string
  icon: LucideIcon
  /** What one source is called, for "5 個設定檔". */
  unit: string
  sources: StepSource[]
  /** What PathKeep reads beyond the listed folders; shown on the left row too. */
  alsoReads?: string
}

const browserIcons: Record<string, string> = {
  Chrome: chromeIcon,
  Edge: edgeIcon,
  Brave: braveIcon,
  Arc: arcIcon,
  Safari: safariIcon,
  Firefox: firefoxIcon,
  Vivaldi: vivaldiIcon,
  Opera: operaIcon,
}

function profile(browser: string, name: string, megabytes: number, blocked = false): StepSource {
  return {
    id: `${browser}-${name}`,
    group: browser,
    icon: <img src={browserIcons[browser]} alt="" className="size-7" />,
    name: browser,
    detail: name,
    amount: bytes(megabytes * 1_000_000),
    amountTitle: `歷史檔案：${bytes(megabytes * 1_000_000)}`,
    blocked,
  }
}

const fewProfiles = [
  profile('Chrome', '個人', 78),
  profile('Chrome', '工作', 18),
  profile('Safari', '預設', 36, true),
  profile('Arc', '預設', 9.8),
  profile('Firefox', 'default-release', 1.4),
]

const manyProfiles = [
  ...['個人', '工作', '客戶 A', '客戶 B', '測試', '家用', '研究', '舊帳號'].map((name, i) => profile('Chrome', name, [78, 18, 6.2, 4.1, 0.9, 12, 3.3, 22][i])),
  ...['預設', '工作', '測試'].map((name, i) => profile('Edge', name, [14, 5.5, 0.4][i])),
  ...['預設', '研究'].map((name, i) => profile('Brave', name, [8.7, 2.2][i])),
  ...['預設', '工作', 'Side project'].map((name, i) => profile('Arc', name, [9.8, 7.1, 1.9][i])),
  profile('Safari', '預設', 36, true),
  ...['default-release', 'dev-edition-default', '工作'].map((name, i) => profile('Firefox', name, [1.4, 0.8, 3.6][i])),
  profile('Vivaldi', '預設', 2.4),
  profile('Opera', '預設', 0.6),
]

const tools: StepSource[] = harnessSources.map((h) => ({
  id: h.id,
  group: h.name,
  icon: <HarnessBadge id={h.id} className="w-7 justify-center" />,
  name: h.name,
  detail: h.alsoReads ? `${h.root} 和 ${h.alsoReads}` : h.root,
  mono: true,
  amount: `${num(h.chats)} 段對話`,
  extra: `${h.subagents ? `另有 ${num(h.subagents)} 個 subagent · ` : ''}${bytes(h.sourceBytes)}`,
}))

const extraToolFolders: StepSource[] = [
  ['claude', '~/work/.claude-config', 214, 1_930, 410],
  ['claude', '/Volumes/Backup/old-mac/.claude/projects', 96, 402, 188],
  ['codex', '/Volumes/Backup/old-mac/.codex/sessions', 133, 0, 96],
  ['codex', '~/clients/acme/.codex/sessions', 41, 12, 22],
  ['pi', '~/work/.pi/agent/sessions', 7, 0, 3.1],
  ['claude', '~/Library/Application Support/Claude/local-agent-mode-sessions', 18, 0, 9.4],
].map(([id, root, chats, subagents, megabytes]) => {
  const tool = harnessSources.find((h) => h.id === id)!
  return {
    id: `${id}-${root}`,
    group: tool.name,
    icon: <HarnessBadge id={tool.id} className="w-7 justify-center" />,
    name: tool.name,
    detail: root as string,
    mono: true,
    amount: `${num(chats as number)} 段對話`,
    extra: `${subagents ? `另有 ${num(subagents as number)} 個 subagent · ` : ''}${bytes((megabytes as number) * 1_000_000)}`,
  }
})

const GIT = '也讀每個專案資料夾的 .git'

function registry(id: KindId, unit: string, sources: StepSource[], alsoReads?: string): StepKind {
  return { id, label: kindById[id].label, icon: kindById[id].icon, unit, sources, alsoReads }
}

/** A planned or made-up type: its sources are names only, the numbers are filler. */
function sketch(id: string, label: string, icon: LucideIcon, names: string[]): StepKind {
  return {
    id,
    label,
    icon,
    unit: '個來源',
    sources: names.map((name, i) => ({
      id: `${id}-${name}`,
      group: name.split(' · ')[0],
      icon: (
        <span className="flex size-7 items-center justify-center rounded-md bg-muted text-[11px] font-semibold text-muted-foreground">
          {name.slice(0, 1)}
        </span>
      ),
      name,
      detail: '示意，這個類型還在規劃',
      amount: bytes((i + 2) * 3_400_000),
    })),
  }
}

export function stepKinds(chosen: Set<KindId>, many: boolean): StepKind[] {
  const list: StepKind[] = []
  for (const kind of kinds) {
    if (!chosen.has(kind.id)) continue
    if (kind.id === 'browsing') list.push(registry('browsing', '個設定檔', many ? manyProfiles : fewProfiles))
    else if (kind.id === 'agent') list.push(registry('agent', many ? '個資料夾' : '個工具', many ? [...tools, ...extraToolFolders] : tools, GIT))
    else if (kind.id === 'messages') list.push(sketch(kind.id, kind.label, kind.icon, ['Slack · 公司', 'Slack · 開源社群', 'Slack · 客戶', 'Discord', 'Telegram']))
    else list.push(sketch(kind.id, kind.label, kind.icon, kind.sources))
  }
  if (many) {
    list.push(sketch('x-downloads', '下載紀錄（假想）', Download, ['Chrome', 'Safari', 'Firefox']))
    list.push(sketch('x-clipboard', '剪貼簿（假想）', Clipboard, ['macOS 剪貼簿']))
  }
  return list
}

/** Every readable source is chosen to begin with; one the system blocks is not. */
export function initialSelection(list: StepKind[]): Record<string, Set<string>> {
  return Object.fromEntries(list.map((kind) => [kind.id, new Set(kind.sources.filter((s) => !s.blocked).map((s) => s.id))]))
}
