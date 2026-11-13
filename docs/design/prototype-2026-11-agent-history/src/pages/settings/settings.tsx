/**
 * Settings: the app's sub-nav, now in two groups. "PathKeep" holds what
 * every data type shares (security, AI provider, background, storage);
 * "資料類型" starts with managing the types, then one entry per enabled
 * type, drawn from the registry so a new type adds one entry and nothing else.
 */
import {
  Activity,
  Database,
  Info,
  LayoutGrid,
  Lock,
  SlidersHorizontal,
  Sparkles,
  type LucideIcon,
} from 'lucide-react'
import type { ReactNode } from 'react'
import { Link, useParams } from 'react-router-dom'
import { cn } from '@/lib/cn'
import { kindById, kinds, type KindId } from '~/kinds'
import { useProto } from '~/proto-state'
import { AgentSection } from './agent-section'
import { BrowsingSection } from './browsing-section'
import { KindsSection } from './kinds-section'
import {
  AboutSection,
  AiSection,
  BackgroundSection,
  GeneralSection,
  PlannedKindSection,
  SecuritySection,
  StorageSection,
} from './pathkeep-sections'

const shared: { id: string; label: string; icon: LucideIcon; Body: () => ReactNode }[] = [
  { id: 'general', label: '一般', icon: SlidersHorizontal, Body: GeneralSection },
  { id: 'security', label: '安全', icon: Lock, Body: SecuritySection },
  { id: 'ai', label: 'AI', icon: Sparkles, Body: AiSection },
  { id: 'background', label: '背景執行', icon: Activity, Body: BackgroundSection },
  { id: 'storage', label: '儲存空間', icon: Database, Body: StorageSection },
  { id: 'about', label: '關於', icon: Info, Body: AboutSection },
]

function kindBody(id: KindId): () => ReactNode {
  if (id === 'agent') return AgentSection
  if (id === 'browsing') return BrowsingSection
  return function Planned() {
    return <PlannedKindSection kind={kindById[id]} />
  }
}

function NavItem({ to, current, icon: Icon, children }: { to: string; current: boolean; icon: LucideIcon; children: ReactNode }) {
  return (
    <Link
      to={to}
      aria-current={current ? 'page' : undefined}
      className={cn(
        'flex h-9 items-center gap-2.5 rounded-lg px-2.5 text-sm font-medium text-muted-foreground transition-colors hover:text-foreground',
        current && 'bg-muted text-foreground',
      )}
    >
      <Icon className="size-4" strokeWidth={1.75} aria-hidden />
      <span className="truncate">{children}</span>
    </Link>
  )
}

function GroupLabel({ children }: { children: ReactNode }) {
  return <span className="px-2.5 pt-4 pb-1.5 text-[11px] font-semibold tracking-wide text-muted-foreground/80 first:pt-0">{children}</span>
}

export default function SettingsPage() {
  const { section } = useParams()
  const { enabledKinds } = useProto()
  const typeEntries = kinds.filter((k) => enabledKinds.includes(k.id))
  const current = section ?? 'general'
  const sharedHit = shared.find((s) => s.id === current)
  const kindHit = typeEntries.find((k) => k.id === current)
  const Body = sharedHit ? sharedHit.Body : current === 'kinds' ? KindsSection : kindHit ? kindBody(kindHit.id) : GeneralSection
  const shownId = sharedHit || current === 'kinds' || kindHit ? current : 'general'

  return (
    <div className="flex min-w-0 flex-1">
      <nav aria-label="設定分類" className="thin-scroll flex w-[220px] shrink-0 flex-col gap-0.5 overflow-y-auto border-r px-3.5 py-7">
        <h1 className="px-2.5 pb-3.5 text-[22px] font-semibold tracking-[-0.01em]">設定</h1>
        <GroupLabel>PathKeep</GroupLabel>
        {shared.map((s) => (
          <NavItem key={s.id} to={`/settings/${s.id}`} current={shownId === s.id} icon={s.icon}>
            {s.label}
          </NavItem>
        ))}
        <GroupLabel>資料類型</GroupLabel>
        <NavItem to="/settings/kinds" current={shownId === 'kinds'} icon={LayoutGrid}>
          管理資料類型
        </NavItem>
        {typeEntries.map((k) => (
          <NavItem key={k.id} to={`/settings/${k.id}`} current={shownId === k.id} icon={k.icon}>
            {k.label}
          </NavItem>
        ))}
        <div className="flex-1" />
        <span className="px-2.5 pt-4 font-mono text-[11px] text-muted-foreground">PathKeep 0.6.0-dev</span>
      </nav>
      <div className="min-w-0 flex-1 overflow-y-auto">
        <div key={shownId} className="flex max-w-[720px] animate-rise flex-col px-10 pt-8 pb-12">
          <Body />
        </div>
      </div>
    </div>
  )
}
