/**
 * Settings › 排程與背景工作: settings only. The 自動備份 card (today on the
 * Backup page, `src/features/backup/schedule-card.tsx`), then the rows of
 * today's Settings › Background work that are choices about background work
 * itself (`src/features/settings/background/`), then a way to the 背景任務 page,
 * which now holds everything there that is status. The link-preview choices
 * that were there concern browsing history only and sit in Settings › 瀏覽紀錄.
 */
import { ListChecks } from 'lucide-react'
import { Link } from 'react-router-dom'
import { SettingRow, SettingsSection } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { QueueControls } from '../schedule/background-controls'
import { ScheduleCard } from '../schedule/schedule-card'
import { ScheduleSwitcher } from '../schedule/schedule-switcher'
import { Group } from './parts'

export function BackgroundSection() {
  return (
    <SettingsSection title="排程與背景工作">
      <ScheduleSwitcher />
      <ScheduleCard />
      <Group title="背景工作" intro="每次備份後，PathKeep 會更新洞察；如果你開啟了，也會更新搜尋索引和網頁摘要。備份從不等這些工作完成。">
        <QueueControls />
      </Group>
      <SettingRow
        title="背景任務"
        description="正在跑、在排隊、今天做完或沒做完的任務。左下角的指示器也能打開。"
        control={
          <Button variant="outline" size="sm" asChild>
            <Link to="/tasks">
              <ListChecks />
              打開背景任務
            </Link>
          </Button>
        }
      />
    </SettingsSection>
  )
}
