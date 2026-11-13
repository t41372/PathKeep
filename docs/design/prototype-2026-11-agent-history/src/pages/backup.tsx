/**
 * Backup: one page for every data type. Header with "back up now", the
 * tools' own auto-delete settings (shown, never changed), sources grouped by
 * type, the shared schedule, archive size by type and recent runs.
 */
import { HardDriveDownload } from 'lucide-react'
import { toast } from 'sonner'
import { PageHeader, PageScroll } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import { NOW } from '~/data/fixture'
import { when } from '~/data/format'
import { AutoDeleteNotice, RunsCard, ScheduleCard, StorageCard } from './backup/cards'
import { SourcesCard } from './backup/sources-card'

export default function BackupPage() {
  return (
    <PageScroll>
      <PageHeader
        title="備份"
        subtitle="PathKeep 定時把每個來源複製進加密存檔。只讀來源，不改也不刪。"
        actions={
          <>
            <span className="flex items-center gap-1.5 rounded-full border bg-card px-3 py-1 text-xs text-muted-foreground shadow-card">
              <span className="size-1.5 rounded-full bg-green" />
              上次備份：{when(NOW - 2 * 3600_000 + 20 * 60_000)}
            </span>
            <Button onClick={() => toast('原型：開始備份所有來源')}>
              <HardDriveDownload />
              立即備份
            </Button>
          </>
        }
      />
      <AutoDeleteNotice />
      <SourcesCard />
      <div className="grid grid-cols-1 gap-5 lg:grid-cols-2">
        <ScheduleCard />
        <StorageCard />
      </div>
      <RunsCard />
    </PageScroll>
  )
}
