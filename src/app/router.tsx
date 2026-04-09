import { Navigate, type RouteObject } from 'react-router-dom'
import { OnboardingShell } from './onboarding-shell'
import { RequireLockScreen, RequireUnlockedShell } from './route-guards'
import { AppShell } from './shell'

export type AppRouteId =
  | 'dashboard'
  | 'explorer'
  | 'insights'
  | 'assistant'
  | 'import'
  | 'audit'
  | 'schedule'
  | 'security'
  | 'settings'
  | 'onboarding'

export type NavigationSection = 'CORE' | 'OPERATIONS' | 'SYSTEM'

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

interface RouteHandle {
  screen: AppScreen
}

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

export const onboardingScreen: AppScreen = {
  id: 'onboarding',
  labelKey: 'navigation.onboardingLabel',
  titleKey: 'navigation.onboardingTitle',
  subtitleKey: 'navigation.onboardingSubtitle',
  icon: '◌',
  href: '/onboarding',
}

export const appScreens = [...appShellScreens, onboardingScreen]

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
    path: 'schedule',
    lazy: async () => {
      const module = await import('../pages/schedule')
      return { Component: module.SchedulePage }
    },
    handle: withHandle(appShellScreens[6]),
  },
  {
    path: 'security',
    lazy: async () => {
      const module = await import('../pages/security')
      return { Component: module.SecurityPage }
    },
    handle: withHandle(appShellScreens[7]),
  },
  {
    path: 'settings',
    lazy: async () => {
      const module = await import('../pages/settings')
      return { Component: module.SettingsPage }
    },
    handle: withHandle(appShellScreens[8]),
  },
]

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
