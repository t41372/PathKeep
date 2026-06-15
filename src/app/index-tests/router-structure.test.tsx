/**
 * @file router-structure.test.tsx
 * @description Structure-only app-shell slice for sidebar grouping and router-handle contracts.
 * @module app/index-tests
 *
 * ## Responsibilities
 * - Preserve the exact sidebar information architecture grouping contract.
 * - Verify desktop-router creation keeps the onboarding route handle readable.
 * - Assert every shell route wires the shared error boundary, including one
 *   render-crash path that proves the boundary on a non-Core route recovers.
 * - Reuse the shared shell reset contract so split suites stay behavior-identical.
 *
 * ## Not responsible for
 * - Rendering route content or onboarding/dashboard flows.
 * - Defining shell metadata; this suite only verifies the existing contract.
 * - Recreating backend reset logic outside the shared test helper.
 *
 * ## Dependencies
 * - Depends on `../router` for sidebar sections, onboarding metadata, and route handles.
 * - Depends on `../router-factory` for the desktop router contract.
 * - Depends on `../index` for the provider stack used by the render-crash case.
 * - Depends on `./test-helpers` for the shared app-shell reset and seed behavior.
 *
 * ## Performance notes
 * - Keeps structure assertions snapshot-based; only one case mounts the shell,
 *   so suite splitting does not multiply render or bootstrap work.
 */

import { isValidElement, type ReactElement } from 'react'
import { render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, test, vi } from 'vitest'
import { createMemoryRouter, type RouteObject } from 'react-router-dom'
import App from '../index'
// Imported statically (not via the per-test `await import('../router')`) so the
// crash case shares one module graph with `App`. `beforeEach` runs
// `vi.resetModules()`, and a dynamically re-imported `appRoutes` would resolve a
// fresh `ShellDataContext` that `App`'s provider never populates — the guard
// would then throw `useShellData` before the route under test even renders.
import { appRoutes as productionAppRoutes, type AppScreen } from '../router'
import { commonT, resetAppShellHarness, seedArchiveRun } from './test-helpers'

const expectedAppShellScreens: AppScreen[] = [
  {
    id: 'dashboard',
    labelKey: 'navigation.dashboardLabel',
    titleKey: 'navigation.dashboardTitle',
    subtitleKey: 'navigation.dashboardSubtitle',
    icon: 'bar_chart',
    href: '/',
    section: 'CORE',
  },
  {
    id: 'explorer',
    labelKey: 'navigation.explorerLabel',
    titleKey: 'navigation.explorerTitle',
    subtitleKey: 'navigation.explorerSubtitle',
    icon: 'auto_stories',
    href: '/explorer',
    section: 'CORE',
  },
  {
    id: 'search',
    labelKey: 'navigation.searchLabel',
    titleKey: 'navigation.searchTitle',
    subtitleKey: 'navigation.searchSubtitle',
    icon: 'search',
    href: '/search',
    section: 'CORE',
  },
  {
    id: 'intelligence',
    labelKey: 'navigation.intelligenceLabel',
    titleKey: 'navigation.intelligenceTitle',
    subtitleKey: 'navigation.intelligenceSubtitle',
    icon: 'memory',
    href: '/intelligence',
    section: 'CORE',
  },
  {
    id: 'assistant',
    labelKey: 'navigation.assistantLabel',
    titleKey: 'navigation.assistantTitle',
    subtitleKey: 'navigation.assistantSubtitle',
    icon: 'smart_toy',
    href: '/assistant',
    badgeKey: 'navigation.assistantBadge',
    section: 'CORE',
  },
  {
    id: 'import',
    labelKey: 'navigation.importLabel',
    titleKey: 'navigation.importTitle',
    subtitleKey: 'navigation.importSubtitle',
    icon: 'download',
    href: '/import',
    section: 'OPERATIONS',
  },
  {
    id: 'audit',
    labelKey: 'navigation.auditLabel',
    titleKey: 'navigation.auditTitle',
    subtitleKey: 'navigation.auditSubtitle',
    icon: 'history',
    href: '/audit',
    section: 'OPERATIONS',
  },
  {
    id: 'jobs',
    labelKey: 'navigation.jobsLabel',
    titleKey: 'navigation.jobsTitle',
    subtitleKey: 'navigation.jobsSubtitle',
    icon: 'database',
    href: '/jobs',
    section: 'OPERATIONS',
  },
  {
    id: 'schedule',
    labelKey: 'navigation.scheduleLabel',
    titleKey: 'navigation.scheduleTitle',
    subtitleKey: 'navigation.scheduleSubtitle',
    icon: 'sync',
    href: '/schedule',
    section: 'SYSTEM',
  },
  {
    id: 'security',
    labelKey: 'navigation.securityLabel',
    titleKey: 'navigation.securityTitle',
    subtitleKey: 'navigation.securitySubtitle',
    icon: 'shield',
    href: '/security',
    section: 'SYSTEM',
  },
  {
    id: 'settings',
    labelKey: 'navigation.settingsLabel',
    titleKey: 'navigation.settingsTitle',
    subtitleKey: 'navigation.settingsSubtitle',
    icon: 'settings',
    href: '/settings',
    section: 'SYSTEM',
  },
  {
    id: 'integrations',
    labelKey: 'navigation.integrationsLabel',
    titleKey: 'navigation.integrationsTitle',
    subtitleKey: 'navigation.integrationsSubtitle',
    icon: 'cloud_upload',
    href: '/integrations',
    section: 'OPERATIONS',
  },
  {
    id: 'maintenance',
    labelKey: 'navigation.maintenanceLabel',
    titleKey: 'navigation.maintenanceTitle',
    subtitleKey: 'navigation.maintenanceSubtitle',
    icon: 'build',
    href: '/maintenance',
    section: 'SYSTEM',
  },
]

const expectedOnboardingScreen: AppScreen = {
  id: 'onboarding',
  labelKey: 'navigation.onboardingLabel',
  titleKey: 'navigation.onboardingTitle',
  subtitleKey: 'navigation.onboardingSubtitle',
  icon: 'check',
  href: '/onboarding',
}

describe('App shell', () => {
  beforeEach(() => {
    vi.resetModules()
    resetAppShellHarness()
  })

  test('keeps sidebar information architecture grouped by section', async () => {
    const {
      appScreens,
      appRoutes,
      findAppScreen,
      onboardingScreen,
      readRouteHandle,
      sidebarSections,
    } = await import('../router')

    expect(appScreens).toEqual([
      ...expectedAppShellScreens,
      expectedOnboardingScreen,
    ])
    expect(sidebarSections).toEqual([
      {
        id: 'core',
        labelKey: 'navigation.coreSection',
        items: expectedAppShellScreens.filter(
          (screen) => screen.section === 'CORE',
        ),
      },
      {
        id: 'operations',
        labelKey: 'navigation.operationsSection',
        items: expectedAppShellScreens.filter(
          (screen) => screen.section === 'OPERATIONS',
        ),
      },
      {
        id: 'system',
        labelKey: 'navigation.systemSection',
        items: expectedAppShellScreens.filter(
          (screen) => screen.section === 'SYSTEM',
        ),
      },
    ])
    expect(onboardingScreen).toEqual(expectedOnboardingScreen)
    expect(findAppScreen('search')).toEqual(expectedAppShellScreens[2])
    expect(routeDescriptors(appRoutes, readRouteHandle)).toEqual([
      {
        path: '/',
        index: false,
        lazy: false,
        errorBoundary: false,
        element: 'RequireUnlockedShell(AppShell)',
        hydrateFallback: 'RouteHydrateFallback',
        handleId: null,
        children: [
          {
            path: 'index',
            index: true,
            lazy: true,
            errorBoundary: true,
            element: null,
            hydrateFallback: null,
            handleId: 'dashboard',
            children: [],
          },
          {
            path: 'explorer',
            index: false,
            lazy: true,
            errorBoundary: true,
            element: null,
            hydrateFallback: null,
            handleId: 'explorer',
            children: [],
          },
          {
            path: 'search',
            index: false,
            lazy: true,
            errorBoundary: true,
            element: null,
            hydrateFallback: null,
            handleId: 'search',
            children: [],
          },
          {
            path: 'intelligence',
            index: false,
            lazy: false,
            errorBoundary: true,
            element: null,
            hydrateFallback: null,
            handleId: 'intelligence',
            children: [
              {
                path: 'index',
                index: true,
                lazy: true,
                errorBoundary: true,
                element: null,
                hydrateFallback: null,
                handleId: null,
                children: [],
              },
              {
                path: 'domain/:domain',
                index: false,
                lazy: true,
                errorBoundary: true,
                element: null,
                hydrateFallback: null,
                handleId: null,
                children: [],
              },
              {
                path: 'query-family/:familyId',
                index: false,
                lazy: true,
                errorBoundary: true,
                element: null,
                hydrateFallback: null,
                handleId: null,
                children: [],
              },
              {
                path: 'refind/:canonicalUrl',
                index: false,
                lazy: true,
                errorBoundary: true,
                element: null,
                hydrateFallback: null,
                handleId: null,
                children: [],
              },
              {
                path: 'session/:sessionId',
                index: false,
                lazy: true,
                errorBoundary: true,
                element: null,
                hydrateFallback: null,
                handleId: null,
                children: [],
              },
              {
                path: 'trail/:trailId',
                index: false,
                lazy: true,
                errorBoundary: true,
                element: null,
                hydrateFallback: null,
                handleId: null,
                children: [],
              },
              {
                path: 'compare-set/:compareSetId',
                index: false,
                lazy: true,
                errorBoundary: true,
                element: null,
                hydrateFallback: null,
                handleId: null,
                children: [],
              },
              {
                path: 'day/:date',
                index: false,
                lazy: true,
                errorBoundary: true,
                element: null,
                hydrateFallback: null,
                handleId: null,
                children: [],
              },
            ],
          },
          {
            path: 'assistant',
            index: false,
            lazy: true,
            errorBoundary: true,
            element: null,
            hydrateFallback: null,
            handleId: 'assistant',
            children: [],
          },
          {
            path: 'import',
            index: false,
            lazy: true,
            errorBoundary: true,
            element: null,
            hydrateFallback: null,
            handleId: 'import',
            children: [],
          },
          {
            path: 'audit',
            index: false,
            lazy: true,
            errorBoundary: true,
            element: null,
            hydrateFallback: null,
            handleId: 'audit',
            children: [],
          },
          {
            path: 'jobs',
            index: false,
            lazy: true,
            errorBoundary: true,
            element: null,
            hydrateFallback: null,
            handleId: 'jobs',
            children: [],
          },
          {
            path: 'schedule',
            index: false,
            lazy: true,
            errorBoundary: true,
            element: null,
            hydrateFallback: null,
            handleId: 'schedule',
            children: [],
          },
          {
            path: 'integrations',
            index: false,
            lazy: true,
            errorBoundary: true,
            element: null,
            hydrateFallback: null,
            handleId: 'integrations',
            children: [],
          },
          {
            path: 'security',
            index: false,
            lazy: true,
            errorBoundary: true,
            element: null,
            hydrateFallback: null,
            handleId: 'security',
            children: [],
          },
          {
            path: 'maintenance',
            index: false,
            lazy: true,
            errorBoundary: true,
            element: null,
            hydrateFallback: null,
            handleId: 'maintenance',
            children: [],
          },
          {
            path: 'settings',
            index: false,
            lazy: true,
            errorBoundary: true,
            element: null,
            hydrateFallback: null,
            handleId: 'settings',
            children: [],
          },
        ],
      },
      {
        path: '/lock',
        index: false,
        lazy: false,
        errorBoundary: false,
        element: 'RequireLockScreen',
        hydrateFallback: null,
        handleId: null,
        children: [],
      },
      {
        path: '/onboarding',
        index: false,
        lazy: false,
        errorBoundary: false,
        element: 'RequireUnlockedShell(OnboardingShell)',
        hydrateFallback: 'RouteHydrateFallback',
        handleId: 'onboarding',
        children: [
          {
            path: 'index',
            index: true,
            lazy: true,
            errorBoundary: false,
            element: null,
            hydrateFallback: null,
            handleId: 'onboarding',
            children: [],
          },
        ],
      },
      {
        path: '*',
        index: false,
        lazy: false,
        errorBoundary: false,
        element: 'Navigate(to=/,replace=true)',
        hydrateFallback: null,
        handleId: null,
        children: [],
      },
    ])
  })

  test('rejects route handles that reference an unknown screen id', async () => {
    const { findAppScreen } = await import('../router')

    expect(() =>
      findAppScreen('__missing__' as Parameters<typeof findAppScreen>[0]),
    ).toThrow('Unknown app screen id: __missing__')
  })

  test('creates a desktop router and validates route handles', async () => {
    const { onboardingScreen, readRouteHandle } = await import('../router')
    const { createDesktopRouter } = await import('../router-factory')
    const router = createDesktopRouter()

    expect(readRouteHandle(null)).toBeNull()
    expect(readRouteHandle({})).toBeNull()
    expect(readRouteHandle({ screen: null })).toBeNull()
    expect(readRouteHandle({ screen: 'dashboard' })).toBeNull()
    expect(readRouteHandle({ screen: onboardingScreen })).toEqual({
      screen: onboardingScreen,
    })
    expect(router.state.location.pathname).toBe('/')

    router.dispose()
  })

  test('keeps every lazy route importable from the production registry', async () => {
    const { appRoutes } = await import('../router')
    const lazyRoutes = collectLazyRoutes(appRoutes)

    expect(lazyRoutes.map((route) => route.path ?? 'index')).toEqual([
      'index',
      'explorer',
      'search',
      'index',
      'domain/:domain',
      'query-family/:familyId',
      'refind/:canonicalUrl',
      'session/:sessionId',
      'trail/:trailId',
      'compare-set/:compareSetId',
      'day/:date',
      'assistant',
      'import',
      'audit',
      'jobs',
      'schedule',
      'integrations',
      'security',
      'maintenance',
      'settings',
      'index',
    ])

    for (const route of lazyRoutes) {
      const loaded = await route.lazy?.()
      expect(loaded?.Component).toEqual(expect.any(Function))
    }
  })

  test('catches a render crash on a non-Core route with the shell boundary', async () => {
    // `/settings` historically shipped without an ErrorBoundary, so a render
    // crash there bubbled up and took the whole shell to React Router's raw
    // developer error page. Exercise the real registry — only swapping the
    // leaf component for a thrower — to prove the boundary now attached to that
    // route renders the product recovery UI instead of crashing the shell.
    await seedArchiveRun()
    const guardedRoutes = withThrowingLeaf(productionAppRoutes, 'settings')
    const router = createMemoryRouter(guardedRoutes, {
      initialEntries: ['/settings'],
    })

    render(<App router={router} />)

    expect(
      await screen.findByTestId('shell-route-error-boundary'),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('heading', {
        name: commonT('routeRenderErrorTitle'),
      }),
    ).toBeVisible()
    expect(
      screen.queryByText('Unexpected Application Error!'),
    ).not.toBeInTheDocument()

    router.dispose()
  })
})

function ThrowingLeaf(): never {
  throw new Error('settings route exploded')
}

/**
 * Deep-clones the production route tree and replaces the lazy loader of the
 * route with the given `path` with a component that throws on render, leaving
 * every other field — crucially the `ErrorBoundary` wired by the real registry
 * — untouched. This lets a behavior test assert the boundary attached to a
 * specific route actually catches render crashes, instead of only snapshotting
 * the boolean flag.
 */
function withThrowingLeaf(routes: RouteObject[], path: string): RouteObject[] {
  return routes.map((route): RouteObject => {
    if (route.path === path && !route.index) {
      // Drop the lazy loader and mount a synchronous thrower so the render
      // crash surfaces immediately under the route's real ErrorBoundary.
      return { ...route, lazy: undefined, Component: ThrowingLeaf }
    }
    if (route.children) {
      return { ...route, children: withThrowingLeaf(route.children, path) }
    }
    return route
  })
}

interface ImportableLazyRoute {
  path?: string
  lazy: () => Promise<{ Component?: unknown }>
}

function collectLazyRoutes(routes: RouteObject[]): ImportableLazyRoute[] {
  const lazyRoutes: ImportableLazyRoute[] = []

  for (const route of routes) {
    if (route.lazy) {
      if (typeof route.lazy !== 'function') {
        throw new Error(
          `Expected route lazy loader to be callable: ${route.path}`,
        )
      }
      lazyRoutes.push({
        path: route.path,
        lazy: route.lazy as () => Promise<{ Component?: unknown }>,
      })
    }
    if (route.children) {
      lazyRoutes.push(...collectLazyRoutes(route.children))
    }
  }

  return lazyRoutes
}

interface RouteDescriptor {
  path: string
  index: boolean
  lazy: boolean
  errorBoundary: boolean
  element: string | null
  hydrateFallback: string | null
  handleId: string | null
  children: RouteDescriptor[]
}

type ReadRouteHandle = (handle: unknown) => { screen: { id: string } } | null

function routeDescriptors(
  routes: RouteObject[],
  readRouteHandle: ReadRouteHandle,
): RouteDescriptor[] {
  return routes.map((route) => ({
    path: route.index ? 'index' : (route.path ?? ''),
    index: route.index === true,
    lazy: typeof route.lazy === 'function',
    errorBoundary: typeof route.ErrorBoundary === 'function',
    element: describeRouteElement(route.element),
    hydrateFallback: describeRouteElement(route.hydrateFallbackElement),
    handleId: readRouteHandle(route.handle)?.screen.id ?? null,
    children: route.children
      ? routeDescriptors(route.children, readRouteHandle)
      : [],
  }))
}

function describeRouteElement(element: unknown): string | null {
  if (!isValidElement(element)) {
    return null
  }

  const typeName = routeElementTypeName(element.type)

  if (typeName === 'RequireLockScreen') {
    return 'RequireLockScreen'
  }

  if (typeName === 'RouteHydrateFallback') {
    return 'RouteHydrateFallback'
  }

  if (typeName === 'Navigate') {
    return describeNavigateElement(element)
  }

  if (typeName === 'RequireUnlockedShell') {
    return describeUnlockedShellElement(element)
  }

  return `unknown:${typeName}`
}

function routeElementTypeName(type: unknown): string {
  if (typeof type === 'function') {
    const namedType = type as { displayName?: string; name?: string }
    return namedType.displayName ?? namedType.name ?? 'anonymous'
  }
  return String(type)
}

function describeNavigateElement(element: ReactElement<unknown>): string {
  const props = element.props as { replace?: unknown; to?: unknown }
  return `Navigate(to=${String(props.to)},replace=${String(props.replace)})`
}

function describeUnlockedShellElement(element: ReactElement<unknown>): string {
  const { children: child } = element.props as { children?: unknown }
  if (!isValidElement(child)) {
    return 'RequireUnlockedShell(null)'
  }

  const childTypeName = routeElementTypeName(child.type)

  if (childTypeName === 'AppShell') {
    return 'RequireUnlockedShell(AppShell)'
  }

  if (childTypeName === 'OnboardingShell') {
    return 'RequireUnlockedShell(OnboardingShell)'
  }

  return `RequireUnlockedShell(unknown:${childTypeName})`
}
