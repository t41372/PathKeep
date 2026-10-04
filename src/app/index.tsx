/**
 * App root: providers, then whichever full-screen state the session is in.
 * Only the `ready` state mounts the router and the screens.
 */
import { QueryClientProvider } from '@tanstack/react-query'
import { useMemo } from 'react'
import { RouterProvider } from 'react-router-dom'
import { Toaster } from '@/components/ui/sonner'
import { TooltipProvider } from '@/components/ui/tooltip'
import { I18nProvider } from '@/lib/i18n'
import { queryClient } from '@/lib/query'
import { ThemeProvider } from '@/lib/theme'
import { BackupRunnerProvider } from './backup-runner'
import { createAppRouter, createOnboardingRouter } from './router'
import { SessionProvider, useSession } from './session'
import { LockScreen } from './shell/lock-screen'
import {
  BootErrorScreen,
  BootScreen,
  RecoveryScreen,
  UpgradeScreen,
} from './shell/status-screens'

function SessionGate() {
  const { phase, error } = useSession()
  const appRouter = useMemo(
    () => (phase === 'ready' ? createAppRouter() : null),
    [phase],
  )
  const onboardingRouter = useMemo(
    () => (phase === 'onboarding' ? createOnboardingRouter() : null),
    [phase],
  )

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
                <SessionGate />
                <Toaster position="bottom-right" />
              </BackupRunnerProvider>
            </SessionProvider>
          </TooltipProvider>
        </I18nProvider>
      </ThemeProvider>
    </QueryClientProvider>
  )
}
