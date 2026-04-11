import type { DashboardSnapshot } from '../types'
import { call } from './shared'

export const dashboardClient = {
  getSnapshot: () => call<DashboardSnapshot>('load_dashboard_snapshot'),
}
