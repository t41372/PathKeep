/**
 * Settings → About: which build this is, updates, logs, the archive health
 * check, and rebuilding Insights. The places to go when something is wrong.
 */
import { useMutation } from '@tanstack/react-query'
import { useState } from 'react'
import { toast } from 'sonner'
import { useSession } from '@/app/session'
import { SettingRow, SettingsSection } from '@/components/app/setting-row'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
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
      <RebuildInsightsRow />
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

function RebuildInsightsRow() {
  const { t } = useI18n()
  const [confirm, setConfirm] = useState(false)
  const rebuild = useMutation({
    mutationFn: () =>
      intelligenceClient.queueCoreIntelligenceRebuild({ fullRebuild: true }),
    onSuccess: () => toast.success(t('settingsAbout.insights.started')),
    onError: (error) =>
      toast.error(t('settingsAbout.insights.failed'), {
        description: describeError(error, 'queue_core_intelligence_rebuild'),
      }),
  })
  return (
    <SettingRow
      title={t('settingsAbout.insights.title')}
      description={t('settingsAbout.insights.description')}
      control={
        <Button
          size="sm"
          variant="outline"
          disabled={rebuild.isPending}
          onClick={() => setConfirm(true)}
        >
          {t('settingsAbout.insights.action')}
        </Button>
      }
    >
      <AlertDialog open={confirm} onOpenChange={setConfirm}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t('settingsAbout.insights.confirmTitle')}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t('settingsAbout.insights.confirmBody')}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t('common.cancel')}</AlertDialogCancel>
            <AlertDialogAction onClick={() => rebuild.mutate()}>
              {t('settingsAbout.insights.action')}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </SettingRow>
  )
}
