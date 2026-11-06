/**
 * What PathKeep shows where a screen failed to render: what went wrong and a
 * reload, in the user's language, with the error written to the log
 * (`recordRenderError`). Every screen has it: the app's screens and
 * onboarding through their router's `errorElement`, and what the session
 * shows outside the routers (lock, boot, boot error, recovery, upgrade)
 * through `SessionErrorBoundary`. Without it React Router shows its own
 * English error page, and a render error outside the routers leaves the
 * window blank.
 *
 * The view fills what it replaces: the app's panel when one screen failed
 * (the navigation stays), the whole window otherwise. Its backdrop drags the
 * window, as the frame's does.
 *
 * Not responsible for: a card that fails (`SectionCard` keeps the error in
 * the card), or a query that fails (`CardError`, `StateMessage`).
 */
import { Component, useEffect, type ReactNode } from 'react'
import { useRouteError } from 'react-router-dom'
import { backdropDragRegion } from '@/components/app/drag-region'
import { Button } from '@/components/ui/button'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { recordRenderError } from '@/lib/runtime-diagnostics'

function ScreenError({ error }: { error: unknown }) {
  const { t } = useI18n()
  return (
    <div
      {...backdropDragRegion()}
      className="flex h-full flex-1 items-center justify-center p-10"
    >
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

/** A route's `errorElement`. */
export function RouteError() {
  const error = useRouteError()
  useEffect(() => recordRenderError('route-render', error), [error])
  return <ScreenError error={error} />
}

/** Around what the session shows, routers included, so nothing fails blank. */
export class SessionErrorBoundary extends Component<
  { children: ReactNode },
  { error: unknown }
> {
  state = { error: null as unknown }

  static getDerivedStateFromError(error: unknown) {
    return { error }
  }

  componentDidCatch(error: unknown) {
    recordRenderError('session-render', error)
  }

  render() {
    if (this.state.error === null) return this.props.children
    return <ScreenError error={this.state.error} />
  }
}
