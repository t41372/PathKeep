/**
 * This module is the canonical route registry for the desktop shell, including sidebar metadata and route handles.
 *
 * Why this file exists:
 * - Files under `src/app/` explain how the desktop shell is stitched together before route-specific UI takes over.
 * - This is where shared profile scope, app-lock gating, route metadata, and shell-level loading grammar should stay readable.
 *
 * Main declarations:
 * - `AppRouteId`
 * - `NavigationSection`
 * - `AppScreen`
 * - `onboardingScreen`
 * - `appScreens`
 * - `sidebarSections`
 * - `appRoutes`
 * - `readRouteHandle`
 *
 * Source-of-truth notes:
 * - Keep this aligned with `docs/design/screens-and-nav.md` for information architecture and route semantics.
 * - Keep busy, locked, degraded, and loading behavior aligned with `docs/design/ux-principles.md`.
 */

import { Navigate, type RouteObject } from 'react-router-dom'
import { OnboardingShell } from './onboarding-shell'
import { RouteHydrateFallback } from './route-hydrate-fallback'
import { RequireLockScreen, RequireUnlockedShell } from './route-guards'
import { AppShell } from './shell'
import { ShellRouteErrorBoundary } from './shell-route-error-boundary'
import type { GlyphIconName } from '../components/ui'

/**
 * Defines the type-level contract for app route id.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
export type AppRouteId =
  | 'dashboard'
  | 'explorer'
  | 'search'
  | 'intelligence'
  | 'assistant'
  | 'import'
  | 'audit'
  | 'jobs'
  | 'schedule'
  | 'integrations'
  | 'security'
  | 'maintenance'
  | 'settings'
  | 'onboarding'

/**
 * Defines the type-level contract for navigation section.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
export type NavigationSection = 'CORE' | 'OPERATIONS' | 'SYSTEM'

/**
 * Defines the typed shape for app screen.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
export interface AppScreen {
  id: AppRouteId
  titleKey: string
  labelKey: string
  subtitleKey: string
  icon: GlyphIconName
  href: string
  badgeKey?: string
  section?: NavigationSection
}

/**
 * Defines the typed shape for route handle.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
interface RouteHandle {
  screen: AppScreen
}

/**
 * Collects the screen metadata that the shell uses for navigation and routing.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
const appShellScreens: AppScreen[] = [
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

/**
 * Exposes the shared onboarding screen declaration used by this module.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
export const onboardingScreen: AppScreen = {
  id: 'onboarding',
  labelKey: 'navigation.onboardingLabel',
  titleKey: 'navigation.onboardingTitle',
  subtitleKey: 'navigation.onboardingSubtitle',
  icon: 'check',
  href: '/onboarding',
}

/**
 * Collects the screen metadata that the shell uses for navigation and routing.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
export const appScreens = [...appShellScreens, onboardingScreen]

/**
 * Looks up an `AppScreen` by its stable id. Use this from the router so
 * inserting / reordering entries in `appShellScreens` doesn't silently
 * desync `appShellScreens[N]` callsites — the bug pattern that bit us
 * when the `search` entry landed between `explorer` and `intelligence`.
 */
function screen(id: AppRouteId): AppScreen {
  const match = appScreens.find((entry) => entry.id === id)
  if (!match) {
    throw new Error(`Unknown app screen id: ${id}`)
  }
  return match
}

/**
 * Groups navigation metadata into the sidebar sections shown by the shell.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
export const sidebarSections = [
  {
    id: 'core',
    labelKey: 'navigation.coreSection',
    items: appShellScreens.filter((screen) => screen.section === 'CORE'),
  },
  {
    id: 'operations',
    labelKey: 'navigation.operationsSection',
    items: appShellScreens.filter((screen) => screen.section === 'OPERATIONS'),
  },
  {
    id: 'system',
    labelKey: 'navigation.systemSection',
    items: appShellScreens.filter((screen) => screen.section === 'SYSTEM'),
  },
]

/**
 * Explains how with handle works.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
function withHandle(screen: AppScreen): RouteHandle {
  return { screen }
}

const appRouteChildren: RouteObject[] = [
  {
    index: true,
    lazy: async () => {
      const module = await import('../pages/dashboard')
      return { Component: module.DashboardPage }
    },
    handle: withHandle(screen('dashboard')),
  },
  {
    path: 'explorer',
    ErrorBoundary: ShellRouteErrorBoundary,
    lazy: async () => {
      const module = await import('../pages/explorer')
      return { Component: module.ExplorerPage }
    },
    handle: withHandle(screen('explorer')),
  },
  {
    // The search surface currently lives as a `?surface=search` mode
    // inside the Explorer page; the dedicated /search route is the
    // sidebar-grade entry that lifts it out without duplicating the
    // page tree. Future work: extract PaperSearchView into its own
    // standalone page (the design handoff has `pk-search.jsx` as a
    // first-class surface) so this route does not need the query-param
    // bounce. Mirror handle from the search nav screen so the topbar
    // title reads "Search" rather than "History Explorer".
    path: 'search',
    ErrorBoundary: ShellRouteErrorBoundary,
    element: <Navigate replace to="/explorer?surface=search" />,
    handle: withHandle(screen('search')),
  },
  {
    path: 'intelligence',
    handle: withHandle(screen('intelligence')),
    ErrorBoundary: ShellRouteErrorBoundary,
    children: [
      {
        index: true,
        ErrorBoundary: ShellRouteErrorBoundary,
        lazy: async () => {
          const module = await import('../pages/intelligence')
          return { Component: module.IntelligencePage }
        },
      },
      {
        path: 'domain/:domain',
        ErrorBoundary: ShellRouteErrorBoundary,
        lazy: async () => {
          const module = await import('../pages/intelligence')
          return { Component: module.DomainDeepDiveRoutePage }
        },
      },
      {
        path: 'query-family/:familyId',
        ErrorBoundary: ShellRouteErrorBoundary,
        lazy: async () => {
          const module = await import('../pages/intelligence')
          return { Component: module.QueryFamilyInsightsRoutePage }
        },
      },
      {
        path: 'refind/:canonicalUrl',
        ErrorBoundary: ShellRouteErrorBoundary,
        lazy: async () => {
          const module = await import('../pages/intelligence')
          return { Component: module.RefindPageInsightsRoutePage }
        },
      },
      {
        path: 'session/:sessionId',
        ErrorBoundary: ShellRouteErrorBoundary,
        lazy: async () => {
          const module = await import('../pages/intelligence')
          return { Component: module.SessionInsightsRoutePage }
        },
      },
      {
        path: 'trail/:trailId',
        ErrorBoundary: ShellRouteErrorBoundary,
        lazy: async () => {
          const module = await import('../pages/intelligence')
          return { Component: module.TrailInsightsRoutePage }
        },
      },
      {
        path: 'compare-set/:compareSetId',
        ErrorBoundary: ShellRouteErrorBoundary,
        lazy: async () => {
          const module = await import('../pages/intelligence')
          return { Component: module.CompareSetInsightsRoutePage }
        },
      },
      {
        path: 'day/:date',
        ErrorBoundary: ShellRouteErrorBoundary,
        lazy: async () => {
          const module = await import('../pages/intelligence')
          return { Component: module.DayInsightsRoutePage }
        },
      },
    ],
  },
  {
    path: 'assistant',
    lazy: async () => {
      const module = await import('../pages/assistant')
      return { Component: module.AssistantPage }
    },
    handle: withHandle(screen('assistant')),
  },
  {
    path: 'import',
    lazy: async () => {
      const module = await import('../pages/import')
      return { Component: module.ImportPage }
    },
    handle: withHandle(screen('import')),
  },
  {
    path: 'audit',
    lazy: async () => {
      const module = await import('../pages/audit')
      return { Component: module.AuditPage }
    },
    handle: withHandle(screen('audit')),
  },
  {
    path: 'jobs',
    ErrorBoundary: ShellRouteErrorBoundary,
    lazy: async () => {
      const module = await import('../pages/jobs')
      return { Component: module.JobsPage }
    },
    handle: withHandle(screen('jobs')),
  },
  {
    path: 'schedule',
    lazy: async () => {
      const module = await import('../pages/schedule')
      return { Component: module.SchedulePage }
    },
    handle: withHandle(screen('schedule')),
  },
  {
    path: 'integrations',
    lazy: async () => {
      const module = await import('../pages/integrations')
      return { Component: module.IntegrationsPage }
    },
    handle: withHandle(screen('integrations')),
  },
  {
    path: 'security',
    lazy: async () => {
      const module = await import('../pages/security')
      return { Component: module.SecurityPage }
    },
    handle: withHandle(screen('security')),
  },
  {
    path: 'maintenance',
    lazy: async () => {
      const module = await import('../pages/maintenance')
      return { Component: module.MaintenancePage }
    },
    handle: withHandle(screen('maintenance')),
  },
  {
    path: 'settings',
    lazy: async () => {
      const module = await import('../pages/settings')
      return { Component: module.SettingsPage }
    },
    handle: withHandle(screen('settings')),
  },
]

/**
 * Defines the route tree or route registry used by the desktop shell.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
export const appRoutes: RouteObject[] = [
  {
    path: '/',
    element: (
      <RequireUnlockedShell>
        <AppShell />
      </RequireUnlockedShell>
    ),
    hydrateFallbackElement: <RouteHydrateFallback />,
    children: appRouteChildren,
  },
  {
    path: '/lock',
    element: <RequireLockScreen />,
  },
  {
    path: '/onboarding',
    element: (
      <RequireUnlockedShell>
        <OnboardingShell />
      </RequireUnlockedShell>
    ),
    hydrateFallbackElement: <RouteHydrateFallback />,
    handle: withHandle(onboardingScreen),
    children: [
      {
        index: true,
        lazy: async () => {
          const module = await import('../pages/onboarding')
          return { Component: module.OnboardingPage }
        },
        handle: withHandle(onboardingScreen),
      },
    ],
  },
  {
    path: '*',
    element: <Navigate replace to="/" />,
  },
]

/**
 * Reads route handle from the current runtime.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
export function readRouteHandle(handle: unknown): RouteHandle | null {
  if (
    typeof handle === 'object' &&
    handle !== null &&
    'screen' in handle &&
    typeof handle.screen === 'object' &&
    handle.screen !== null
  ) {
    return handle as RouteHandle
  }

  return null
}
