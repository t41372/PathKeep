/**
 * Step 1: why PathKeep exists. Browsers' default retention against
 * PathKeep's, and three promises. Static content; nothing is read yet.
 */
import { Copy, HardDrive, Search, type LucideIcon } from 'lucide-react'
import { BrandMark } from '@/components/app/brand-mark'
import { BrowserIcon } from '@/lib/browser-icons'
import { useI18n, type MessageKey } from '@/lib/i18n'

const retention: { name: string; keep: MessageKey; width: string }[] = [
  { name: 'Chrome', keep: 'onboarding.welcome.days90', width: '8%' },
  { name: 'Arc', keep: 'onboarding.welcome.days90', width: '8%' },
  { name: 'Safari', keep: 'onboarding.welcome.year', width: '32%' },
  { name: 'Firefox', keep: 'onboarding.welcome.bySize', width: '55%' },
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
              <BrowserIcon
                browserName={row.name === 'Chrome' ? 'Google Chrome' : row.name}
                className="size-5"
                decorative
              />
              <span className="w-20 text-[13px]">{row.name}</span>
              <span className="h-2 flex-1 overflow-hidden rounded bg-muted">
                <span
                  className="block h-full rounded bg-foreground/75"
                  style={{ width: row.width }}
                />
              </span>
              <span className="w-24 text-right font-mono text-xs text-muted-foreground">
                {t(row.keep)}
              </span>
            </li>
          ))}
          <li className="flex items-center gap-3 border-t pt-3">
            <BrandMark className="size-5" />
            <span className="w-20 text-[13px] font-semibold">PathKeep</span>
            <span className="h-2 flex-1 rounded bg-brand" />
            <span className="w-24 text-right font-mono text-xs font-medium text-brand">
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
