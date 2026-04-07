import { createHashRouter } from 'react-router-dom'
import { appRoutes } from './router'

export type AppRouter = ReturnType<typeof createHashRouter>

export function createDesktopRouter() {
  return createHashRouter(appRoutes)
}
