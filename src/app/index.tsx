/**
 * This module composes the long-lived providers that every route in the desktop shell depends on.
 *
 * Why this file exists:
 * - It is the narrow place where i18n, shared profile scope, shell bootstrap data, and the router are wired together.
 * - If startup or provider order feels confusing, this file should let you answer that question in one pass.
 *
 * Main declarations:
 * - `AppBody`
 * - `App`
 *
 * Source-of-truth notes:
 * - Keep the surrounding shell contract aligned with `docs/design/screens-and-nav.md` and `docs/design/ux-principles.md`.
 * - This file should stay thin; route and page logic belongs in `src/app/` and `src/pages/`, not in the app entry wrapper.
 */

import { RouterProvider } from 'react-router-dom'
import { ArchiveRecoveryScreen } from '../components/archive-recovery-screen'
import { ArchiveUpgradeScreen } from '../components/archive-upgrade-screen'
import { I18nProvider } from '../lib/i18n'
import { ProfileScopeProvider } from '../lib/profile-scope'
import { createDesktopRouter, type AppRouter } from './router-factory'
import { ShellDataProvider } from './shell-data'
import { useShellData } from './shell-data-context'

/**
 * Describes the props accepted by `App`.
 *
 * Tests can override the router so they exercise the real provider stack
 * without having to boot the default desktop router for every scenario.
 */
interface AppProps {
  router?: AppRouter
}

/**
 * Renders the router or, in precedence order, a full-screen gate in its place:
 * the `ArchiveRecoveryScreen` when the archive cannot open on launch and
 * recovery snapshots are available, or the `ArchiveUpgradeScreen` when a
 * healthy archive is version-behind and the one-time upgrade migration is
 * pending. The provider stack is identical in every case so shell-data hooks
 * work inside each gate.
 */
export function AppBody({ router }: { router: AppRouter }) {
  const { recovery, archiveUpgrade } = useShellData()
  if (recovery) return <ArchiveRecoveryScreen report={recovery} />
  if (archiveUpgrade)
    return (
      <ArchiveUpgradeScreen
        assessment={archiveUpgrade.assessment}
        config={archiveUpgrade.config}
      />
    )
  return <RouterProvider router={router} />
}

/**
 * Renders the root provider tree for the desktop shell.
 *
 * The provider order matters because every route expects translations, shared
 * profile scope, and shell bootstrap data to already exist before React Router
 * takes over.
 */
export default function App({ router = createDesktopRouter() }: AppProps) {
  return (
    <I18nProvider>
      <ProfileScopeProvider>
        <ShellDataProvider>
          <AppBody router={router} />
        </ShellDataProvider>
      </ProfileScopeProvider>
    </I18nProvider>
  )
}
