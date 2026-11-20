/**
 * Settings: a sub-nav on the left, the chosen section on the right. The
 * section comes from the route (`/settings/:section`), so other screens can
 * link straight to one. The sub-nav has two groups (`data-spaces.md` §8):
 * what PathKeep as a whole does, and the kinds of data it keeps.
 *
 * Not responsible for any setting itself; each section file owns its rows.
 */
import {
  Activity,
  Bot,
  Database,
  Globe,
  Info,
  Layers,
  Lock,
  Sparkles,
  SlidersHorizontal,
  type LucideIcon,
} from 'lucide-react'
import { Link, useParams } from 'react-router-dom'
import { dragRegion } from '@/components/app/drag-region'
import { TitleBarStrip } from '@/components/app/title-bar'
import { useSession } from '@/app/session'
import { cn } from '@/lib/cn'
import { useI18n, type MessageKey } from '@/lib/i18n'
import { useAgentKind } from '@/lib/queries/agent-backup'
import { useKindState } from '@/lib/queries/data-kinds'
import { AboutSection } from './about-section'
import { AgentSection } from './agent/agent-section'
import { AiSection } from './ai-section'
import { BackgroundSection } from './background/background-section'
import { BrowsingSection } from './browsing/browsing-section'
import { GeneralSection } from './general-section'
import { KindsSection } from './kinds/kinds-section'
import { SecuritySection } from './security-section'
import { StorageSection } from './storage-section'

const sections = {
  general: { icon: SlidersHorizontal, Body: GeneralSection },
  security: { icon: Lock, Body: SecuritySection },
  ai: { icon: Sparkles, Body: AiSection },
  background: { icon: Activity, Body: BackgroundSection },
  storage: { icon: Database, Body: StorageSection },
  about: { icon: Info, Body: AboutSection },
  kinds: { icon: Layers, Body: KindsSection },
  browsing: { icon: Globe, Body: BrowsingSection },
  agent: { icon: Bot, Body: AgentSection },
} satisfies Record<string, { icon: LucideIcon; Body: () => React.ReactNode }>

type SectionId = keyof typeof sections

const groups = [
  {
    label: 'settings.nav.groupApp',
    ids: ['general', 'security', 'ai', 'background', 'storage', 'about'],
  },
  { label: 'settings.nav.groupKinds', ids: ['kinds', 'browsing', 'agent'] },
] as const satisfies readonly {
  label: MessageKey
  ids: readonly SectionId[]
}[]

function isSection(value: string | undefined): value is SectionId {
  return value !== undefined && value in sections
}

export default function SettingsPage() {
  const { t } = useI18n()
  const { buildInfo } = useSession()
  const params = useParams()
  const agent = useAgentKind().row?.state
  const browsing = useKindState('browsing')
  // A kind's own page is listed while it is on or paused (`data-spaces.md` §8).
  const listed = (id: SectionId) =>
    id === 'agent'
      ? agent !== undefined && agent !== 'off'
      : id !== 'browsing' || browsing !== 'off'
  const current: SectionId = isSection(params.section)
    ? params.section
    : 'general'
  const Body = sections[current].Body

  return (
    <div className="flex min-w-0 flex-1">
      <nav
        {...dragRegion()}
        aria-label={t('settings.nav.label')}
        className="flex w-[220px] shrink-0 flex-col gap-0.5 border-r px-3.5 py-7"
      >
        <h1 className="px-2.5 pb-3.5 text-[22px] font-semibold tracking-[-0.01em]">
          {t('settings.title')}
        </h1>
        {groups.map((group, index) => (
          <div
            key={group.label}
            role="group"
            aria-labelledby={`settings-group-${index}`}
            className={cn('flex flex-col gap-0.5', index > 0 && 'mt-4')}
          >
            <h2
              id={`settings-group-${index}`}
              className="px-2.5 pb-1 text-xs font-medium text-muted-foreground"
            >
              {t(group.label)}
            </h2>
            {group.ids.filter(listed).map((id) => {
              const Icon = sections[id].icon
              return (
                <Link
                  key={id}
                  to={`/settings/${id}`}
                  aria-current={id === current ? 'page' : undefined}
                  className={cn(
                    'flex h-9 items-center gap-2.5 rounded-lg px-2.5 text-sm font-medium text-muted-foreground transition-colors hover:text-foreground',
                    id === current && 'bg-muted text-foreground',
                  )}
                >
                  <Icon className="size-4" strokeWidth={1.75} aria-hidden />
                  {t(`settings.nav.${id}`)}
                </Link>
              )
            })}
          </div>
        ))}
        <div className="flex-1" />
        {buildInfo && (
          <span className="px-2.5 font-mono text-[11px] text-muted-foreground">
            {buildInfo.productName} {buildInfo.version}
          </span>
        )}
      </nav>
      <div className="min-w-0 flex-1 overflow-y-auto">
        <TitleBarStrip />
        <div
          key={current}
          className="flex max-w-[680px] animate-rise flex-col px-10 pt-8 pb-12"
        >
          <Body />
        </div>
      </div>
    </div>
  )
}
