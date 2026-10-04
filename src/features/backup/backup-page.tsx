/**
 * Backup: what PathKeep copies, when it runs, how older history comes in,
 * and what recent runs did. Each card owns its own data and actions, except
 * the two import cards: a file dropped anywhere on the window belongs to
 * exactly one of them, so the page owns their state and routes each drop.
 */
import { BackupNowButton } from '@/components/app/backup-button'
import { PageHeader, PageScroll } from '@/components/app/section-card'
import { useBackupRunner } from '@/app/backup-runner'
import { useI18n } from '@/lib/i18n'
import { BrowserImportCard } from './browser-import-card'
import { dropTarget, useWindowFileDrop } from './file-drop'
import { ImportCard } from './import-card'
import { ImportsCard } from './imports-card'
import { ProgressCard } from './progress-card'
import { RunsCard } from './runs-card'
import { ScheduleCard } from './schedule-card'
import { SourcesCard } from './sources-card'
import { useBrowserImport } from './use-browser-import'
import { useTakeoutImport } from './use-takeout-import'

export default function BackupPage() {
  const { t } = useI18n()
  const backup = useBackupRunner()
  const takeout = useTakeoutImport()
  const browser = useBrowserImport()
  const takeoutIdle = takeout.state.step === 'idle'
  const browserIdle = browser.state.step === 'idle'
  const hovering = useWindowFileDrop(
    (path) => {
      if (dropTarget(path) === 'browser') {
        if (browserIdle) void browser.inspect(path, 'detect')
      } else if (takeoutIdle) void takeout.inspect(path)
    },
    (takeoutIdle || browserIdle) && !backup.running,
  )

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
        <ImportCard takeout={takeout} hovering={hovering && takeoutIdle} />
        <BrowserImportCard
          state={browser.state}
          hovering={hovering && browserIdle}
          onInspect={(path, source) => void browser.inspect(path, source)}
          onConfirm={() => void browser.confirm()}
          onReset={browser.reset}
        />
        <ImportsCard />
      </div>
      <RunsCard />
    </PageScroll>
  )
}
