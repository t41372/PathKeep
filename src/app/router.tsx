import { Navigate, type RouteObject } from 'react-router-dom'
import { DashboardPage } from '../pages/dashboard'
import { ExplorerPage } from '../pages/explorer'
import { InsightsPage } from '../pages/insights'
import { AssistantPage } from '../pages/assistant'
import { ImportPage } from '../pages/import'
import { AuditPage } from '../pages/audit'
import { SchedulePage } from '../pages/schedule'
import { SecurityPage } from '../pages/security'
import { SettingsPage } from '../pages/settings'
import { OnboardingPage } from '../pages/onboarding'
import { OnboardingShell } from './onboarding-shell'
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
  title: string
  label: string
  subtitle: string
  icon: string
  href: string
  badge?: string
  section?: NavigationSection
}

interface RouteHandle {
  screen: AppScreen
}

const appShellScreens: AppScreen[] = [
  {
    id: 'dashboard',
    label: 'Dashboard',
    title: 'Dashboard',
    subtitle: 'Archive overview & system status',
    icon: '⌂',
    href: '/',
    section: 'CORE',
  },
  {
    id: 'explorer',
    label: 'Explorer',
    title: 'History Explorer',
    subtitle: 'Browse, search & filter your archive',
    icon: '◎',
    href: '/explorer',
    section: 'CORE',
  },
  {
    id: 'insights',
    label: 'Insights',
    title: 'Insights',
    subtitle: 'Topics, threads & browsing patterns',
    icon: '◈',
    href: '/insights',
    section: 'CORE',
  },
  {
    id: 'assistant',
    label: 'AI Assistant',
    title: 'AI Assistant',
    subtitle: 'Ask questions about your browsing history',
    icon: '▷',
    href: '/assistant',
    badge: 'OPT',
    section: 'CORE',
  },
  {
    id: 'import',
    label: 'Import',
    title: 'Import',
    subtitle: 'Google Takeout & browser direct import',
    icon: '↓',
    href: '/import',
    section: 'OPERATIONS',
  },
  {
    id: 'audit',
    label: 'Audit Ledger',
    title: 'Audit Ledger',
    subtitle: 'Manifest chain, run history & integrity',
    icon: '⊞',
    href: '/audit',
    section: 'OPERATIONS',
  },
  {
    id: 'schedule',
    label: 'Schedule',
    title: 'Schedule',
    subtitle: 'Backup schedule & install artifacts',
    icon: '⏀',
    href: '/schedule',
    section: 'OPERATIONS',
  },
  {
    id: 'security',
    label: 'Security',
    title: 'Security',
    subtitle: 'Encryption, keyring & password management',
    icon: '⊘',
    href: '/security',
    section: 'SYSTEM',
  },
  {
    id: 'settings',
    label: 'Settings',
    title: 'Settings',
    subtitle: 'Profiles, AI provider & general config',
    icon: '⚙',
    href: '/settings',
    section: 'SYSTEM',
  },
]

export const onboardingScreen: AppScreen = {
  id: 'onboarding',
  label: 'Onboarding',
  title: 'Onboarding / Setup',
  subtitle: 'Preview, manual guidance, and first-run archive decisions',
  icon: '◌',
  href: '/onboarding',
}

export const appScreens = [...appShellScreens, onboardingScreen]

export const sidebarSections = [
  {
    label: 'CORE',
    items: appShellScreens.filter((screen) => screen.section === 'CORE'),
  },
  {
    label: 'OPERATIONS',
    items: appShellScreens.filter((screen) => screen.section === 'OPERATIONS'),
  },
  {
    label: 'SYSTEM',
    items: appShellScreens.filter((screen) => screen.section === 'SYSTEM'),
  },
]

function withHandle(screen: AppScreen): RouteHandle {
  return { screen }
}

const appRouteChildren: RouteObject[] = [
  {
    index: true,
    element: <DashboardPage />,
    handle: withHandle(appShellScreens[0]),
  },
  {
    path: 'explorer',
    element: <ExplorerPage />,
    handle: withHandle(appShellScreens[1]),
  },
  {
    path: 'insights',
    element: <InsightsPage />,
    handle: withHandle(appShellScreens[2]),
  },
  {
    path: 'assistant',
    element: <AssistantPage />,
    handle: withHandle(appShellScreens[3]),
  },
  {
    path: 'import',
    element: <ImportPage />,
    handle: withHandle(appShellScreens[4]),
  },
  {
    path: 'audit',
    element: <AuditPage />,
    handle: withHandle(appShellScreens[5]),
  },
  {
    path: 'schedule',
    element: <SchedulePage />,
    handle: withHandle(appShellScreens[6]),
  },
  {
    path: 'security',
    element: <SecurityPage />,
    handle: withHandle(appShellScreens[7]),
  },
  {
    path: 'settings',
    element: <SettingsPage />,
    handle: withHandle(appShellScreens[8]),
  },
]

export const appRoutes: RouteObject[] = [
  {
    path: '/',
    element: <AppShell />,
    children: appRouteChildren,
  },
  {
    path: '/onboarding',
    element: <OnboardingShell />,
    handle: withHandle(onboardingScreen),
    children: [
      {
        index: true,
        element: <OnboardingPage />,
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
