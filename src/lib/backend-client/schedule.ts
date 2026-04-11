import type { ApplyResult, SchedulePlan, ScheduleStatus } from '../types'
import { call } from './shared'

export const scheduleClient = {
  previewInstall: (platform?: string) =>
    call<SchedulePlan>('preview_schedule', { platform }),
  getStatus: (platform?: string) => call<ScheduleStatus>('schedule_status', { platform }),
  applyInstall: (plan: SchedulePlan) => call<ApplyResult>('apply_schedule', { plan }),
  removeInstall: (plan: SchedulePlan) => call<ApplyResult>('remove_schedule', { plan }),
}
