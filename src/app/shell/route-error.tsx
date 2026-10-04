/** Per-screen error fallback, so one broken screen never takes down the app. */
import { useRouteError } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'

export function RouteError() {
  const { t } = useI18n()
  const error = useRouteError()
  return (
    <div className="flex flex-1 items-center justify-center p-10">
      <div className="flex max-w-md flex-col items-center gap-3 text-center">
        <h2 className="font-semibold">{t('shell.error.route')}</h2>
        <pre className="max-h-40 w-full overflow-auto rounded-lg bg-muted p-3 text-left font-mono text-xs whitespace-pre-wrap text-muted-foreground">
          {describeError(error)}
        </pre>
        <Button variant="outline" onClick={() => window.location.reload()}>
          {t('shell.error.reload')}
        </Button>
      </div>
    </div>
  )
}
