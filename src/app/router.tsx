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
import { RequireLockScreen, RequireUnlockedShell } from './route-guards'
import { AppShell } from './shell'

/**
 * Defines the type-level contract for app route id.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
export type AppRouteId =
  | 'dashboard'
  | 'explorer'
  | 'insights'
  | 'assistant'
  | 'import'
  | 'audit'
  | 'jobs'
  | 'schedule'
  | 'security'
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
  icon: string
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
    icon: '⌂',
    href: '/',
    section: 'CORE',
  },
  {
    id: 'explorer',
    labelKey: 'navigation.explorerLabel',
    titleKey: 'navigation.explorerTitle',
    subtitleKey: 'navigation.explorerSubtitle',
    icon: '◎',
    href: '/explorer',
    section: 'CORE',
  },
  {
    id: 'insights',
    labelKey: 'navigation.insightsLabel',
    titleKey: 'navigation.insightsTitle',
    subtitleKey: 'navigation.insightsSubtitle',
    icon: '◈',
    href: '/insights',
    section: 'CORE',
  },
  {
    id: 'assistant',
    labelKey: 'navigation.assistantLabel',
    titleKey: 'navigation.assistantTitle',
    subtitleKey: 'navigation.assistantSubtitle',
    icon: '▷',
    href: '/assistant',
    badgeKey: 'navigation.assistantBadge',
    section: 'CORE',
  },
  {
    id: 'import',
    labelKey: 'navigation.importLabel',
    titleKey: 'navigation.importTitle',
    subtitleKey: 'navigation.importSubtitle',
    icon: '↓',
    href: '/import',
    section: 'OPERATIONS',
  },
  {
    id: 'audit',
    labelKey: 'navigation.auditLabel',
    titleKey: 'navigation.auditTitle',
    subtitleKey: 'navigation.auditSubtitle',
    icon: '⊞',
    href: '/audit',
    section: 'OPERATIONS',
  },
  {
    id: 'jobs',
    labelKey: 'navigation.jobsLabel',
    titleKey: 'navigation.jobsTitle',
    subtitleKey: 'navigation.jobsSubtitle',
    icon: '≡',
    href: '/jobs',
    section: 'OPERATIONS',
  },
  {
    id: 'schedule',
    labelKey: 'navigation.scheduleLabel',
    titleKey: 'navigation.scheduleTitle',
    subtitleKey: 'navigation.scheduleSubtitle',
    icon: '⏀',
    href: '/schedule',
    section: 'OPERATIONS',
  },
  {
    id: 'security',
    labelKey: 'navigation.securityLabel',
    titleKey: 'navigation.securityTitle',
    subtitleKey: 'navigation.securitySubtitle',
    icon: '⊘',
    href: '/security',
    section: 'SYSTEM',
  },
  {
    id: 'settings',
    labelKey: 'navigation.settingsLabel',
    titleKey: 'navigation.settingsTitle',
    subtitleKey: 'navigation.settingsSubtitle',
    icon: '⚙',
    href: '/settings',
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
  icon: '◌',
  href: '/onboarding',
}

/**
 * Collects the screen metadata that the shell uses for navigation and routing.
 *
 * The shell layer owns routing, app-lock boundaries, shared scope, and bootstrap read-model logic, so small named declarations here prevent the shell from turning into a single opaque blob.
 */
export const appScreens = [...appShellScreens, onboardingScreen]

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
    handle: withHandle(appShellScreens[0]),
  },
  {
    path: 'explorer',
    lazy: async () => {
      const module = await import('../pages/explorer')
      return { Component: module.ExplorerPage }
    },
    handle: withHandle(appShellScreens[1]),
  },
  {
    path: 'insights',
    lazy: async () => {
      const module = await import('../pages/insights')
      return { Component: module.InsightsPage }
    },
    handle: withHandle(appShellScreens[2]),
  },
  {
    path: 'assistant',
    lazy: async () => {
      const module = await import('../pages/assistant')
      return { Component: module.AssistantPage }
    },
    handle: withHandle(appShellScreens[3]),
  },
  {
    path: 'import',
    lazy: async () => {
      const module = await import('../pages/import')
      return { Component: module.ImportPage }
    },
    handle: withHandle(appShellScreens[4]),
  },
  {
    path: 'audit',
    lazy: async () => {
      const module = await import('../pages/audit')
      return { Component: module.AuditPage }
    },
    handle: withHandle(appShellScreens[5]),
  },
  {
    path: 'jobs',
    lazy: async () => {
      const module = await import('../pages/jobs')
      return { Component: module.JobsPage }
    },
    handle: withHandle(appShellScreens[6]),
  },
  {
    path: 'schedule',
    lazy: async () => {
      const module = await import('../pages/schedule')
      return { Component: module.SchedulePage }
    },
    handle: withHandle(appShellScreens[7]),
  },
  {
    path: 'security',
    lazy: async () => {
      const module = await import('../pages/security')
      return { Component: module.SecurityPage }
    },
    handle: withHandle(appShellScreens[8]),
  },
  {
    path: 'settings',
    lazy: async () => {
      const module = await import('../pages/settings')
      return { Component: module.SettingsPage }
    },
    handle: withHandle(appShellScreens[9]),
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
