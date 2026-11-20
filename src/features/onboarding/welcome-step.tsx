/**
 * Step 1: why PathKeep exists. How long browsers and Agent tools keep
 * things by default against PathKeep's forever, and three promises. Static
 * content; nothing is read yet.
 */
import { Copy, HardDrive, Search, type LucideIcon } from 'lucide-react'
import type { ReactNode } from 'react'
import { BrandMark } from '@/components/app/brand-mark'
import { ToolIcon } from '@/features/data-kinds/tool-badge'
import { BrowserIcon } from '@/lib/browser-icons'
import { useI18n, type MessageKey } from '@/lib/i18n'

const browser = (name: string) => (
  <BrowserIcon browserName={name} className="size-5" decorative />
)

/** Shortest first; the bar is roughly how long each keeps things. */
const retention: {
  name: string
  icon: ReactNode
  keep: MessageKey
  width: string
}[] = [
  {
    name: 'Claude Code',
    icon: <ToolIcon id="claude-code" />,
    keep: 'onboarding.welcome.claudeCode',
    width: '3%',
  },
  {
    name: 'Chrome',
    icon: browser('Google Chrome'),
    keep: 'onboarding.welcome.days90',
    width: '8%',
  },
  {
    name: 'Arc',
    icon: browser('Arc'),
    keep: 'onboarding.welcome.days90',
    width: '8%',
  },
  {
    name: 'Safari',
    icon: browser('Safari'),
    keep: 'onboarding.welcome.year',
    width: '32%',
  },
  {
    name: 'Firefox',
    icon: browser('Firefox'),
    keep: 'onboarding.welcome.bySize',
    width: '55%',
  },
  {
    name: 'Codex',
    icon: <ToolIcon id="codex" />,
    keep: 'onboarding.welcome.codex',
    width: '64%',
  },
]

const promises: { icon: LucideIcon; key: 'local' | 'originals' | 'search' }[] =
  [
    { icon: HardDrive, key: 'local' },
    { icon: Copy, key: 'originals' },
    { icon: Search, key: 'search' },
  ]

export function WelcomeStep() {
  const { t } = useI18n()
  return (
    <>
      <section className="flex flex-col gap-3.5 rounded-xl border bg-card p-5 shadow-card">
        <h2 className="text-[13px] font-normal text-muted-foreground">
          {t('onboarding.welcome.retentionTitle')}
        </h2>
        <ul className="flex flex-col gap-3.5">
          {retention.map((row) => (
            <li key={row.name} className="flex items-center gap-3">
              <span className="flex size-5 shrink-0 items-center justify-center">
                {row.icon}
              </span>
              <span className="w-24 shrink-0 text-[13px]">{row.name}</span>
              <span className="h-2 flex-1 overflow-hidden rounded bg-muted">
                <span
                  className="block h-full rounded bg-foreground/75"
                  style={{ width: row.width }}
                />
              </span>
              <span className="w-[168px] shrink-0 text-right font-mono text-xs text-muted-foreground">
                {t(row.keep)}
              </span>
            </li>
          ))}
          <li className="flex items-center gap-3 border-t pt-3">
            <BrandMark className="size-5" />
            <span className="w-24 shrink-0 text-[13px] font-semibold">
              PathKeep
            </span>
            <span className="h-2 flex-1 rounded bg-brand" />
            <span className="w-[168px] shrink-0 text-right font-mono text-xs font-medium text-brand">
              {t('onboarding.welcome.forever')}
            </span>
          </li>
        </ul>
      </section>
      <ul className="grid grid-cols-3 gap-2.5">
        {promises.map(({ icon: Icon, key }) => (
          <li
            key={key}
            className="flex flex-col gap-1.5 rounded-[10px] bg-muted p-3.5"
          >
            <Icon className="size-[18px]" strokeWidth={1.75} aria-hidden />
            <span className="text-[13px] font-medium">
              {t(`onboarding.welcome.${key}.title`)}
            </span>
            <span className="text-xs leading-snug text-muted-foreground">
              {t(`onboarding.welcome.${key}.body`)}
            </span>
          </li>
        ))}
      </ul>
    </>
  )
}
