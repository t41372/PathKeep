/**
 * Settings › 排程與背景工作: settings only. When PathKeep backs up by itself
 * (the 自動備份 card, moved here from the Backup page) and how background work
 * runs. What is running right now is status and lives on the 背景任務 page.
 */
import { ListChecks } from 'lucide-react'
import { useState } from 'react'
import { Link } from 'react-router-dom'
import { SettingRow, SettingsSection } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { ScheduleCard } from '../backup/cards'
import { Group } from './parts'

function Toggle({ initial, label }: { initial: boolean; label: string }) {
  const [on, setOn] = useState(initial)
  return <Switch checked={on} onCheckedChange={setOn} aria-label={label} />
}

export function BackgroundSection() {
  return (
    <SettingsSection title="排程與背景工作">
      {/* Settings only: when PathKeep backs up by itself, then how background work runs.
          What is running right now is status and lives on the 背景任務 page. */}
      <ScheduleCard />
      <Group title="背景工作" intro="備份之後更新搜尋索引、洞察這類工作。">
        <SettingRow title="低電量模式時暫停" control={<Toggle initial label="低電量模式時暫停" />} />
        <SettingRow
          title="現在在跑什麼"
          description="正在跑、在排隊、今天做完或需要你看一下的任務，都在背景任務頁。左下角的指示器也能打開。"
          control={
            <Button variant="outline" size="sm" asChild>
              <Link to="/tasks">
                <ListChecks />
                打開背景任務
              </Link>
            </Button>
          }
        />
      </Group>
    </SettingsSection>
  )
}
