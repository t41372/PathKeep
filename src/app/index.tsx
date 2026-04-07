import { RouterProvider } from 'react-router-dom'
import { createDesktopRouter, type AppRouter } from './router-factory'

interface AppProps {
  router?: AppRouter
}

export default function App({ router = createDesktopRouter() }: AppProps) {
  return <RouterProvider router={router} />
}
