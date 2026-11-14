/**
 * Routes. Each screen is code-split so the first paint only loads the shell.
 * A space's pages live under `#/<space>/<page>` and come from the registry
 * (`src/lib/spaces`); Ask and Settings are shared and carry no space.
 */
import { lazy, Suspense, type ComponentType } from 'react'
import { createHashRouter, Navigate, type RouteObject } from 'react-router-dom'
import { Skeleton } from '@/components/ui/skeleton'
import { readLastSpace, spaces, spaceStartPath } from '@/lib/spaces'
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

function spaceRoutes(): RouteObject[] {
  return spaces.flatMap((space) =>
    space.pages.flatMap((page) => {
      const base = `${space.id}/${page.id}`
      const children = 'children' in page ? page.children : []
      return [
        { path: base, element: screen(page.screen) },
        ...children.map((child) => ({
          path: `${base}/${child.path}`,
          element: screen(child.screen),
        })),
      ]
    }),
  )
}

function OpenLastSpace() {
  return <Navigate to={spaceStartPath(readLastSpace())} replace />
}

export function createAppRouter() {
  return createHashRouter([
    {
      element: <AppFrame />,
      // The frame itself failed: the error takes the window.
      errorElement: <RouteError />,
      children: [
        {
          // One screen failed: the error takes its place in the frame, the
          // navigation stays, and going to another screen clears it.
          errorElement: <RouteError />,
          children: [
            // `#/` opens the space the user was last in.
            { index: true, element: <OpenLastSpace /> },
            ...spaceRoutes(),
            {
              path: 'ask',
              element: screen(() => import('@/features/ask/ask-page')),
            },
            {
              path: 'settings/:section?',
              element: screen(
                () => import('@/features/settings/settings-page'),
              ),
            },
            { path: '*', element: <Navigate to="/" replace /> },
          ],
        },
      ],
    },
  ])
}

export function createOnboardingRouter() {
  return createHashRouter([
    {
      path: '*',
      element: screen(() => import('@/features/onboarding/onboarding-page')),
      errorElement: <RouteError />,
    },
  ])
}
