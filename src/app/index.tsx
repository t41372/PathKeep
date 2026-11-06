/**
 * App root: providers, then whichever full-screen state the session is in.
 * Only the `ready` state mounts the router and the screens. A screen that
 * fails to render, in any state, shows the error view (`route-error.tsx`)
 * instead of leaving the window blank. The quit question sits beside them, so
 * it can show whatever state the session is in.
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
import { useIndexJobEvents } from './index-job-events'
import { createAppRouter, createOnboardingRouter } from './router'
import { SessionProvider, useSession } from './session'
import { LockScreen } from './shell/lock-screen'
import { QuitPrompt } from './shell/quit-prompt'
import { SessionErrorBoundary } from './shell/route-error'
import {
  BootErrorScreen,
  BootScreen,
  RecoveryScreen,
  SchemaUpdateScreen,
  UpgradeScreen,
} from './shell/status-screens'
import { useSchemaUpdate } from './schema-update'

function SessionGate() {
  const { phase, error } = useSession()
  const schemaUpdate = useSchemaUpdate()
  useIndexJobEvents(phase === 'ready')
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
    case 'app-locked':
      return <LockScreen kind="app" />
    case 'archive-locked':
      return <LockScreen kind="archive" />
    case 'recovery':
      return <RecoveryScreen />
    case 'upgrade':
      return <UpgradeScreen />
    case 'onboarding':
      return <RouterProvider router={onboardingRouter!} />
    case 'ready':
      return <RouterProvider router={appRouter!} />
  }
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <ThemeProvider>
        <I18nProvider>
          <TooltipProvider delayDuration={400}>
            <SessionProvider>
              <BackupRunnerProvider>
                <ImportRunnerProvider>
                  <SessionErrorBoundary>
                    <SessionGate />
                  </SessionErrorBoundary>
                  <QuitPrompt />
                  <Toaster position="bottom-right" />
                </ImportRunnerProvider>
              </BackupRunnerProvider>
            </SessionProvider>
          </TooltipProvider>
        </I18nProvider>
      </ThemeProvider>
    </QueryClientProvider>
  )
}
