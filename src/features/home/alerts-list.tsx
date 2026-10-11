/**
 * Home's alerts, under the header: one row per problem, each with the place
 * that fixes it. Renders nothing when there are none. Which alerts apply is
 * decided in `alerts.ts`.
 */
import { CircleAlert, TriangleAlert } from 'lucide-react'
import { Link } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { useHomeAlerts } from './alerts'

export function HomeAlerts() {
  const { t } = useI18n()
  const alerts = useHomeAlerts()
  if (alerts.length === 0) return null
  return (
    <section aria-label={t('home.alerts.label')} className="animate-rise">
      <ul className="flex flex-col gap-2">
        {alerts.map((alert) => {
          const Icon = alert.tone === 'error' ? CircleAlert : TriangleAlert
          return (
            <li
              key={alert.id}
              className={cn(
                'flex items-start gap-3 rounded-xl border p-3.5 text-[13px]',
                alert.tone === 'error'
                  ? 'border-destructive/30 bg-destructive/5'
                  : 'border-brand/30 bg-brand-soft',
              )}
            >
              <Icon
                aria-hidden
                className={cn(
                  'mt-0.5 size-4 shrink-0',
                  alert.tone === 'error' ? 'text-destructive' : 'text-brand',
                )}
              />
              <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                <h2 className="font-medium">{alert.title}</h2>
                {alert.detail && (
                  <p className="text-muted-foreground">{alert.detail}</p>
                )}
              </div>
              <div className="flex shrink-0 items-center gap-1.5 max-[640px]:flex-col max-[640px]:items-end">
                {alert.dismiss && (
                  <Button size="sm" variant="ghost" onClick={alert.dismiss.run}>
                    {alert.dismiss.label}
                  </Button>
                )}
                <Button size="sm" variant="outline" asChild>
                  <Link to={alert.action.to}>{alert.action.label}</Link>
                </Button>
              </div>
            </li>
          )
        })}
      </ul>
    </section>
  )
}
