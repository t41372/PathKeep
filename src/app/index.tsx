import { RouterProvider } from 'react-router-dom'
import { createDesktopRouter, type AppRouter } from './router-factory'
import { ShellDataProvider } from './shell-data'

interface AppProps {
  router?: AppRouter
}

export default function App({ router = createDesktopRouter() }: AppProps) {
  return (
    <ShellDataProvider>
      <RouterProvider router={router} />
    </ShellDataProvider>
  )
}
