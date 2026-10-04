/**
 * Settings → About: which build this is, updates, diagnostics (logs, crash
 * reports, a copyable report) and the archive health check. The places to go when something is wrong. (Rebuilding
 * Insights lives in Settings → Background work.)
 */
import { useSession } from '@/app/session'
import { SettingRow, SettingsSection } from '@/components/app/setting-row'
import { Skeleton } from '@/components/ui/skeleton'
import { useI18n } from '@/lib/i18n'
import {
  CrashReportsRow,
  DiagnosticsReportRow,
  LogsRow,
} from './diagnostics-rows'
import { HealthRow } from './health-row'
import { UpdateRow } from './update-row'

export function AboutSection() {
  const { t } = useI18n()
  const { buildInfo } = useSession()
  return (
    <SettingsSection title={t('settings.nav.about')}>
      <SettingRow
        title={t('settingsAbout.version.title')}
        description={
          buildInfo ? (
            <span className="font-mono text-xs">
              {buildInfo.productName} {buildInfo.version} ·{' '}
              {buildInfo.gitCommitShort}
              {buildInfo.gitDirty && ` ${t('settingsAbout.version.modified')}`}
            </span>
          ) : (
            <Skeleton className="mt-0.5 h-3.5 w-48" />
          )
        }
      />
      {buildInfo && <UpdateRow version={buildInfo.version} />}
      <LogsRow />
      <CrashReportsRow />
      <DiagnosticsReportRow />
      <HealthRow />
    </SettingsSection>
  )
}
