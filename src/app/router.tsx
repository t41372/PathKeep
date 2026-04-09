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
  title: string
  titleKey?: string
  label: string
  labelKey?: string
  subtitle: string
  subtitleKey?: string
  icon: string
  href: string
  badge?: string
  badgeKey?: string
  section?: NavigationSection
}

interface RouteHandle {
  screen: AppScreen
}

const appShellScreens: AppScreen[] = [
  {
    id: 'dashboard',
    label: 'Dashboard',
    labelKey: 'navigation.dashboardLabel',
    title: 'Dashboard',
    titleKey: 'navigation.dashboardTitle',
    subtitle: 'Archive overview & system status',
    subtitleKey: 'navigation.dashboardSubtitle',
    icon: '⌂',
    href: '/',
    section: 'CORE',
  },
  {
    id: 'explorer',
    label: 'Explorer',
    labelKey: 'navigation.explorerLabel',
    title: 'History Explorer',
    titleKey: 'navigation.explorerTitle',
    subtitle: 'Browse, search & filter your archive',
    subtitleKey: 'navigation.explorerSubtitle',
    icon: '◎',
    href: '/explorer',
    section: 'CORE',
  },
  {
    id: 'insights',
    label: 'Insights',
    labelKey: 'navigation.insightsLabel',
    title: 'Insights',
    titleKey: 'navigation.insightsTitle',
    subtitle: 'Topics, threads & browsing patterns',
    subtitleKey: 'navigation.insightsSubtitle',
    icon: '◈',
    href: '/insights',
    section: 'CORE',
  },
  {
    id: 'assistant',
    label: 'AI Assistant',
    labelKey: 'navigation.assistantLabel',
    title: 'AI Assistant',
    titleKey: 'navigation.assistantTitle',
    subtitle: 'Ask questions about your browsing history',
    subtitleKey: 'navigation.assistantSubtitle',
    icon: '▷',
    href: '/assistant',
    badge: 'OPT',
    badgeKey: 'navigation.assistantBadge',
    section: 'CORE',
  },
  {
    id: 'import',
    label: 'Import',
    labelKey: 'navigation.importLabel',
    title: 'Import',
    titleKey: 'navigation.importTitle',
    subtitle: 'Google Takeout & browser direct import',
    subtitleKey: 'navigation.importSubtitle',
    icon: '↓',
    href: '/import',
    section: 'OPERATIONS',
  },
  {
    id: 'audit',
    label: 'Audit Ledger',
    labelKey: 'navigation.auditLabel',
    title: 'Audit Ledger',
    titleKey: 'navigation.auditTitle',
    subtitle: 'Manifest chain, run history & integrity',
    subtitleKey: 'navigation.auditSubtitle',
    icon: '⊞',
    href: '/audit',
    section: 'OPERATIONS',
  },
  {
    id: 'schedule',
    label: 'Schedule',
    labelKey: 'navigation.scheduleLabel',
    title: 'Schedule',
    titleKey: 'navigation.scheduleTitle',
    subtitle: 'Backup schedule & install artifacts',
    subtitleKey: 'navigation.scheduleSubtitle',
    icon: '⏀',
    href: '/schedule',
    section: 'OPERATIONS',
  },
  {
    id: 'security',
    label: 'Security',
    labelKey: 'navigation.securityLabel',
    title: 'Security',
    titleKey: 'navigation.securityTitle',
    subtitle: 'Encryption, keyring & password management',
    subtitleKey: 'navigation.securitySubtitle',
    icon: '⊘',
    href: '/security',
    section: 'SYSTEM',
  },
  {
    id: 'settings',
    label: 'Settings',
    labelKey: 'navigation.settingsLabel',
    title: 'Settings',
    titleKey: 'navigation.settingsTitle',
    subtitle: 'Profiles, language & platform guidance',
    subtitleKey: 'navigation.settingsSubtitle',
    icon: '⚙',
    href: '/settings',
    section: 'SYSTEM',
  },
]

export const onboardingScreen: AppScreen = {
  id: 'onboarding',
  label: 'Onboarding',
  labelKey: 'navigation.onboardingLabel',
  title: 'Onboarding / Setup',
  titleKey: 'navigation.onboardingTitle',
  subtitle: 'Preview, manual guidance, and first-run archive decisions',
  subtitleKey: 'navigation.onboardingSubtitle',
  icon: '◌',
  href: '/onboarding',
}

export const appScreens = [...appShellScreens, onboardingScreen]

export const sidebarSections = [
  {
    label: 'CORE',
    labelKey: 'navigation.coreSection',
    items: appShellScreens.filter((screen) => screen.section === 'CORE'),
  },
  {
    label: 'OPERATIONS',
    labelKey: 'navigation.operationsSection',
    items: appShellScreens.filter((screen) => screen.section === 'OPERATIONS'),
  },
  {
    label: 'SYSTEM',
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
