/**
 * Routes. Each screen is code-split so the first paint only loads the shell.
 * A space's pages live under `#/<space>/<page>` and come from the registry
 * (`src/lib/spaces`); every address of a space goes through its kind's
 * state first (`kind-gate.tsx`), so a kind that is off says so at any of
 * its addresses. Ask, background tasks and Settings are shared and carry
 * no space.
 */
import { lazy, Suspense, type ComponentType } from 'react'
import { createHashRouter, Navigate, type RouteObject } from 'react-router-dom'
import {
  hasPages,
  readLastSpace,
  spaces,
  spaceStartPath,
  type Space,
} from '@/lib/spaces'
import { AppFrame } from './shell/app-frame'
import { KindGate } from './shell/kind-gate'
import { RouteError } from './shell/route-error'
import { ScreenFallback } from './shell/screen-fallback'

function screen(load: () => Promise<{ default: ComponentType }>) {
  const Screen = lazy(load)
  return (
    <Suspense fallback={<ScreenFallback />}>
      <Screen />
    </Suspense>
  )
}

function pageRoutes(space: Space): RouteObject[] {
  return space.pages.flatMap((page) => {
    const children = 'children' in page ? (page.children ?? []) : []
    const elsewhere = 'elsewhere' in page ? (page.elsewhere ?? []) : []
    return [
      { path: page.id, element: screen(page.screen) },
      ...children.map((child) => ({
        path: `${page.id}/${child.path}`,
        element: screen(child.screen),
      })),
      ...elsewhere.map((other) => ({
        path: other.path,
        element: screen(other.screen),
      })),
    ]
  })
}

function spaceRoutes(): RouteObject[] {
  return spaces.map((space) => ({
    path: space.id,
    element: <KindGate space={space} />,
    children: [
      ...(hasPages(space) ? pageRoutes(space) : []),
      // `#/<space>` alone, or a page it does not have: the last space.
      { index: true, element: <Navigate to="/" replace /> },
      { path: '*', element: <Navigate to="/" replace /> },
    ],
  }))
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
              path: 'tasks',
              element: screen(() => import('@/features/tasks/tasks-page')),
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
