/**
 * This module composes the long-lived providers that every route in the desktop shell depends on.
 *
 * Why this file exists:
 * - It is the narrow place where i18n, shared profile scope, shell bootstrap data, and the router are wired together.
 * - If startup or provider order feels confusing, this file should let you answer that question in one pass.
 *
 * Main declarations:
 * - `App`
 *
 * Source-of-truth notes:
 * - Keep the surrounding shell contract aligned with `docs/design/screens-and-nav.md` and `docs/design/ux-principles.md`.
 * - This file should stay thin; route and page logic belongs in `src/app/` and `src/pages/`, not in the app entry wrapper.
 */

import { RouterProvider } from 'react-router-dom'
import { I18nProvider } from '../lib/i18n'
import { ProfileScopeProvider } from '../lib/profile-scope'
import { createDesktopRouter, type AppRouter } from './router-factory'
import { ShellDataProvider } from './shell-data'

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
          <RouterProvider router={router} />
        </ShellDataProvider>
      </ProfileScopeProvider>
    </I18nProvider>
  )
}
