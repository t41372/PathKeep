import { RouterProvider } from 'react-router-dom'
import { I18nProvider } from '../lib/i18n'
import { ProfileScopeProvider } from '../lib/profile-scope'
import { createDesktopRouter, type AppRouter } from './router-factory'
import { ShellDataProvider } from './shell-data'

interface AppProps {
  router?: AppRouter
}

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
