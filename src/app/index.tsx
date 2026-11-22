/**
 * App root: providers, then whichever full-screen state the session is in.
 * Only the `ready` state mounts the router and the screens. A screen that
 * fails to render, in any state, shows the error view (`route-error.tsx`)
 * instead of leaving the window blank. The quit question sits beside them, so
 * it can show whatever state the session is in.
 *
 * The window's own buttons (Windows and Linux) sit at this level too, outside
 * every error boundary: they belong to the window, which has no system title
 * bar on any screen, not to one screen's layout. Drawn inside the archive's
 * frame they were missing from onboarding, the lock screens and the rest
 * (`back-forward-window-frame-tradeoff.md` §落地).
 */
import { QueryClientProvider } from '@tanstack/react-query'
import { useMemo } from 'react'
import { RouterProvider } from 'react-router-dom'
import { Toaster } from '@/components/ui/sonner'
import { TooltipProvider } from '@/components/ui/tooltip'
import { I18nProvider } from '@/lib/i18n'
import { queryClient } from '@/lib/query'
import { ThemeProvider } from '@/lib/theme'
import { ImportRunnerProvider } from '@/features/backup/import-runner'
import { BackupRunnerProvider } from './backup-runner'
import { useConfigDurabilityNotice } from './config-durability'
import { useIndexJobEvents } from './index-job-events'
import { createAppRouter, createOnboardingRouter } from './router'
import { SessionProvider, useSession } from './session'
import { AccessScreen } from './shell/archive-access/access-screen'
import { LockScreen } from './shell/lock-screen'
import { QuitPrompt } from './shell/quit-prompt'
import { RenderErrorBoundary } from './shell/route-error'
import {
  BootErrorScreen,
  BootScreen,
  RecoveryScreen,
  SchemaUpdateScreen,
  UpgradeScreen,
} from './shell/status-screens'
import { useSchemaUpdate } from './schema-update'
import { WindowDrag } from './window-chrome/drag'
import { WindowControls } from './window-chrome/window-controls'

function SessionGate() {
  const { phase, error } = useSession()
  const schemaUpdate = useSchemaUpdate()
  useIndexJobEvents(phase === 'ready')
  useConfigDurabilityNotice()
  const appRouter = useMemo(
    () => (phase === 'ready' ? createAppRouter() : null),
    [phase],
  )
  const onboardingRouter = useMemo(
    () => (phase === 'onboarding' ? createOnboardingRouter() : null),
    [phase],
  )

  // A schema update while the archive opens (at launch, from the keychain or
  // an unlock) can take a while on a large archive; show it rather than a
  // spinner. Only on the screens an archive opens behind: onboarding creates
  // a new archive (nothing to copy) and must keep its own state, and the
  // conversion screen shows its own progress.
  if (
    schemaUpdate &&
    (phase === 'booting' ||
      phase === 'archive-locked' ||
      phase === 'app-locked')
  ) {
    return <SchemaUpdateScreen progress={schemaUpdate} />
  }
  switch (phase) {
    case 'booting':
      return <BootScreen />
    case 'error':
      return <BootErrorScreen message={error ?? ''} />
    case 'access':
      return <AccessScreen />
    case 'app-locked':
      return <LockScreen kind="app" />
    case 'archive-locked':
      return <LockScreen kind="archive" />
    case 'recovery':
      return <RecoveryScreen />
    case 'upgrade':
      return <UpgradeScreen />
    case 'onboarding':
      // A provider owns one router's route state for its lifetime. The archive
      // router must mount separately so it starts with its own matched routes.
      return <RouterProvider key="onboarding" router={onboardingRouter!} />
    case 'ready':
      return <RouterProvider key="archive" router={appRouter!} />
  }
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <ThemeProvider>
        <I18nProvider>
          <WindowControls />
          <WindowDrag />
          <TooltipProvider delayDuration={400}>
            <RenderErrorBoundary source="app-render">
              <SessionProvider>
                <BackupRunnerProvider>
                  <ImportRunnerProvider>
                    <RenderErrorBoundary source="session-render">
                      <SessionGate />
                    </RenderErrorBoundary>
                    <QuitPrompt />
                    <Toaster position="bottom-right" />
                  </ImportRunnerProvider>
                </BackupRunnerProvider>
              </SessionProvider>
            </RenderErrorBoundary>
          </TooltipProvider>
        </I18nProvider>
      </ThemeProvider>
    </QueryClientProvider>
  )
}
