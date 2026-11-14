/**
 * Browsing history: the rows that are only about browsing and are not about
 * backing it up. Profiles, their switches and imports stay on the browsing
 * Backup page. The copy is today's; only where the rows sit is proposed.
 */
import { Download } from 'lucide-react'
import { useState } from 'react'
import { Link } from 'react-router-dom'
import { SettingRow, SettingsSection } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { Group, MovedFrom, Note } from './parts'

export function BrowsingSection() {
  const [previews, setPreviews] = useState(true)
  const [summaries, setSummaries] = useState(false)
  return (
    <SettingsSection title="瀏覽紀錄">
      <Note>
        要備份哪些瀏覽器設定檔、匯入 Google Takeout 或瀏覽器的歷史檔，都在瀏覽紀錄的
        <Link to="/browsing/backup" className="mx-1 text-brand hover:underline">
          備份頁
        </Link>
        ，不在這裡。
      </Note>
      <Group title="線上" intro="PathKeep 自己會連到網站的只有這兩項。">
        <SettingRow
          title={
            <>
              連結預覽 <MovedFrom>一般 › 線上</MovedFrom>
            </>
          }
          description="下載每個頁面提供的預覽圖，顯示在紀錄和首頁。"
          control={<Switch checked={previews} onCheckedChange={setPreviews} aria-label="連結預覽" />}
        />
        <SettingRow
          title={
            <>
              預覽圖快取 <MovedFrom>儲存空間</MovedFrom>
            </>
          }
          description="12,480 張 · 1.9 GB"
          control={
            <Button variant="outline" size="sm">
              清除…
            </Button>
          }
        />
        <SettingRow
          title={
            <>
              網頁摘要 <MovedFrom>一般 › 線上</MovedFrom>
            </>
          }
          description="抓取你造訪過的頁面的摘要，讓搜尋也找得到。"
          control={<Switch checked={summaries} onCheckedChange={setSummaries} aria-label="網頁摘要" />}
        />
      </Group>
      <Group title="匯出">
        <SettingRow
          title={
            <>
              匯出瀏覽紀錄 <MovedFrom>儲存空間</MovedFrom>
            </>
          }
          description="整個瀏覽紀錄存成一個檔：HTML、Markdown、純文字或 JSON Lines。"
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
