/**
 * Backup: what PathKeep copies, when it runs, how older history comes in,
 * and what recent runs did. Each card owns its own data and actions.
 */
import { BackupNowButton } from '@/components/app/backup-button'
import { PageHeader, PageScroll } from '@/components/app/section-card'
import { useI18n } from '@/lib/i18n'
import { ImportCard } from './import-card'
import { ProgressCard } from './progress-card'
import { RunsCard } from './runs-card'
import { ScheduleCard } from './schedule-card'
import { SourcesCard } from './sources-card'

export default function BackupPage() {
  const { t } = useI18n()
  return (
    <PageScroll>
      <PageHeader
        title={t('backup.title')}
        subtitle={t('backup.subtitle')}
        actions={<BackupNowButton />}
      />
      <ProgressCard />
      <SourcesCard />
      <div className="grid grid-cols-1 gap-5 lg:grid-cols-2">
        <ScheduleCard />
        <ImportCard />
      </div>
      <RunsCard />
    </PageScroll>
  )
}
