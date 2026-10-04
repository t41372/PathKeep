/**
 * Settings: a sub-nav on the left, the chosen section on the right. The
 * section comes from the route (`/settings/:section`), so other screens can
 * link straight to one.
 *
 * Not responsible for any setting itself; each section file owns its rows.
 */
import {
  Activity,
  Database,
  Info,
  Lock,
  Sparkles,
  SlidersHorizontal,
  type LucideIcon,
} from 'lucide-react'
import { Link, useParams } from 'react-router-dom'
import { useSession } from '@/app/session'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { AboutSection } from './about-section'
import { AiSection } from './ai-section'
import { BackgroundSection } from './background/background-section'
import { GeneralSection } from './general-section'
import { SecuritySection } from './security-section'
import { StorageSection } from './storage-section'

const sections = {
  general: { icon: SlidersHorizontal, Body: GeneralSection },
  security: { icon: Lock, Body: SecuritySection },
  ai: { icon: Sparkles, Body: AiSection },
  background: { icon: Activity, Body: BackgroundSection },
  storage: { icon: Database, Body: StorageSection },
  about: { icon: Info, Body: AboutSection },
} satisfies Record<string, { icon: LucideIcon; Body: () => React.ReactNode }>

type SectionId = keyof typeof sections

const order = Object.keys(sections) as SectionId[]

function isSection(value: string | undefined): value is SectionId {
  return value !== undefined && value in sections
}

export default function SettingsPage() {
  const { t } = useI18n()
  const { buildInfo } = useSession()
  const params = useParams()
  const current: SectionId = isSection(params.section)
    ? params.section
    : 'general'
  const Body = sections[current].Body

  return (
    <div className="flex min-w-0 flex-1">
      <nav
        aria-label={t('settings.nav.label')}
        className="flex w-[220px] shrink-0 flex-col gap-0.5 border-r px-3.5 py-7"
      >
        <h1 className="px-2.5 pb-3.5 text-[22px] font-semibold tracking-[-0.01em]">
          {t('settings.title')}
        </h1>
        {order.map((id) => {
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
        <div className="flex-1" />
        {buildInfo && (
          <span className="px-2.5 font-mono text-[11px] text-muted-foreground">
            {buildInfo.productName} {buildInfo.version}
          </span>
        )}
      </nav>
      <div className="min-w-0 flex-1 overflow-y-auto">
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
