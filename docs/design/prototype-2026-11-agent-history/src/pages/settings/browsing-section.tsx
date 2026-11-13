/** Browsing history: the browser-specific rows that live in Settings today, moved under their type. */
import { Download } from 'lucide-react'
import { useState } from 'react'
import { Link } from 'react-router-dom'
import { SettingRow, SettingsSection } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { num } from '~/data/format'
import { browserSources } from '../backup/sources-data'
import { Group } from './parts'

export function BrowsingSection() {
  const [previews, setPreviews] = useState(true)
  const [summaries, setSummaries] = useState(false)
  const profiles = browserSources.length
  const visits = browserSources.reduce((s, b) => s + b.visits, 0)
  return (
    <SettingsSection title="瀏覽紀錄">
      <Group title="來源">
        <SettingRow
          title="瀏覽器與設定檔"
          description={`偵測到 ${profiles} 個設定檔，共 ${num(visits)} 筆造訪。在「備份」頁可以個別暫停。`}
          control={
            <Button variant="outline" size="sm" asChild>
              <Link to="/backup">到備份頁</Link>
            </Button>
          }
        />
      </Group>
      <Group title="顯示">
        <SettingRow
          title="連結預覽"
          description="在紀錄裡把滑鼠停在網址上，顯示網頁標題與縮圖。需要時才連線抓，抓到的存在本機。"
          control={<Switch checked={previews} onCheckedChange={setPreviews} aria-label="連結預覽" />}
        />
        <SettingRow
          title="頁面摘要"
          description="用你在「AI」裡設定的模型，替常回去的頁面寫一句摘要。"
          control={<Switch checked={summaries} onCheckedChange={setSummaries} aria-label="頁面摘要" />}
        />
      </Group>
      <Group title="匯出">
        <SettingRow
          title="匯出瀏覽紀錄"
          description="存成 CSV 或 JSON，可以選日期範圍與瀏覽器。"
          control={
            <Button variant="outline" size="sm">
              <Download />
              匯出…
            </Button>
          }
        />
      </Group>
    </SettingsSection>
  )
}
