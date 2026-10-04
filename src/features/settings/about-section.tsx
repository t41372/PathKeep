/**
 * Settings → About: which build this is, updates, logs, and the archive
 * health check. The places to go when something is wrong. (Rebuilding
 * Insights lives in Settings → Background work.)
 */
import { useMutation } from '@tanstack/react-query'
import { toast } from 'sonner'
import { useSession } from '@/app/session'
import { SettingRow, SettingsSection } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { supportClient } from '@/lib/backend-client/support'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
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
      <HealthRow />
    </SettingsSection>
  )
}

function LogsRow() {
  const { t } = useI18n()
  const reveal = useMutation({
    mutationFn: supportClient.revealLogs,
    onError: (error) =>
      toast.error(t('settingsAbout.logs.failed'), {
        description: describeError(error, 'reveal_logs'),
      }),
  })
  return (
    <SettingRow
      title={t('settingsAbout.logs.title')}
      description={t('settingsAbout.logs.description')}
      control={
        <Button
          size="sm"
          variant="outline"
          disabled={reveal.isPending}
          onClick={() => reveal.mutate()}
        >
          {t('settingsAbout.logs.action')}
        </Button>
      }
    />
  )
}
