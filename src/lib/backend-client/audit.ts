import type { AuditRunDetail, HealthRepairReport, HealthReport } from '../types'
import { call } from './shared'

export const auditClient = {
  getRunDetail: (runId: number) =>
    call<AuditRunDetail>('load_audit_run_detail', { runId }),
  getHealthReport: () => call<HealthReport>('doctor_report'),
  repairHealth: () => call<HealthRepairReport>('repair_health'),
}
