/** Routes. Each screen is code-split so the first paint only loads the shell. */
import { lazy, Suspense, type ComponentType } from 'react'
import { createHashRouter, Navigate } from 'react-router-dom'
import { Skeleton } from '@/components/ui/skeleton'
import { AppFrame } from './shell/app-frame'
import { RouteError } from './shell/route-error'

function ScreenFallback() {
  return (
    <div className="mx-auto flex w-full max-w-[1040px] flex-col gap-5 px-10 py-9">
      <Skeleton className="h-9 w-64" />
      <div className="grid grid-cols-4 gap-3">
        {Array.from({ length: 4 }, (_, index) => (
          <Skeleton key={index} className="h-24 rounded-xl" />
        ))}
      </div>
      <Skeleton className="h-64 rounded-xl" />
    </div>
  )
}

function screen(load: () => Promise<{ default: ComponentType }>) {
  const Screen = lazy(load)
  return (
    <Suspense fallback={<ScreenFallback />}>
      <Screen />
    </Suspense>
  )
}

export function createAppRouter() {
  return createHashRouter([
    {
      element: <AppFrame />,
      errorElement: <RouteError />,
      children: [
        {
          index: true,
          element: screen(() => import('@/features/home/home-page')),
        },
        {
          path: 'history',
          element: screen(() => import('@/features/history/history-page')),
        },
        {
          path: 'insights',
          element: screen(() => import('@/features/insights/insights-page')),
        },
        {
          path: 'insights/day/:date',
          element: screen(() => import('@/features/insights/day-page')),
        },
        {
          path: 'insights/site/:domain',
          element: screen(() => import('@/features/insights/site-page')),
        },
        {
          path: 'insights/search/:query',
          element: screen(() => import('@/features/insights/search-page')),
        },
        {
          path: 'insights/page/:url',
          element: screen(() => import('@/features/insights/page-page')),
        },
        {
          path: 'ask',
          element: screen(() => import('@/features/ask/ask-page')),
        },
        {
          path: 'backup',
          element: screen(() => import('@/features/backup/backup-page')),
        },
        {
          path: 'settings/:section?',
          element: screen(() => import('@/features/settings/settings-page')),
        },
        { path: '*', element: <Navigate to="/" replace /> },
      ],
    },
  ])
}

export function createOnboardingRouter() {
  return createHashRouter([
    {
      path: '*',
      element: screen(() => import('@/features/onboarding/onboarding-page')),
    },
  ])
}
