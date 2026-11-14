/**
 * Browsing history: the rows that are only about browsing and are not about
 * backing it up. Profiles, their switches and imports stay on the browsing
 * Backup page. Every row and string here is today's (online-rows.tsx,
 * storage-section.tsx's preview images, export-row.tsx); only where the rows
 * sit is proposed.
 */
import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import { toast } from 'sonner'
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
import { Progress } from '@/components/ui/progress'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { num } from '~/data/format'
import { browserSources } from '../backup/sources-data'
import { Group, MovedFrom, Note } from './parts'

const formats = [
  { id: 'html', label: 'HTML' },
  { id: 'markdown', label: 'Markdown' },
  { id: 'text', label: '純文字' },
  { id: 'jsonl', label: 'JSON Lines' },
]

function PreviewImagesRow() {
  const [images, setImages] = useState(12_480)
  const [confirm, setConfirm] = useState(false)
  const size = images ? '1.9 GB' : '0 B'
  return (
    <SettingRow
      title={
        <>
          連結預覽圖 <MovedFrom>儲存空間</MovedFrom>
        </>
      }
      description={`${num(images)} 張圖片，${size}`}
      control={
        <Button size="sm" variant="outline" disabled={images === 0} onClick={() => setConfirm(true)}>
          清除…
        </Button>
      }
    >
      <AlertDialog open={confirm} onOpenChange={setConfirm}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>要刪除所有連結預覽圖嗎？</AlertDialogTitle>
            <AlertDialogDescription>
              將刪除 {num(images)} 張圖片（{size}）。只有開啟連結預覽時，PathKeep 才會重新下載。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                setImages(0)
                toast.success('已釋放 1.9 GB。')
              }}
            >
              清除…
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </SettingRow>
  )
}

function ExportRow() {
  const total = browserSources.reduce((sum, source) => sum + source.visits, 0)
  const [format, setFormat] = useState('html')
  const [written, setWritten] = useState<number | null>(null)
  const running = written !== null
  useEffect(() => {
    if (written === null) return
    if (written >= total) {
      setWritten(null)
      toast.success(`已匯出 ${num(total)} 筆記錄。`)
      return
    }
    const timer = setTimeout(() => setWritten(Math.min(total, written + Math.ceil(total / 12))), 700)
    return () => clearTimeout(timer)
  }, [written, total])
  return (
    <SettingRow
      title={
        <>
          匯出歷史 <MovedFrom>儲存空間</MovedFrom>
        </>
      }
      description="把你的全部歷史存成一個檔案"
      control={
        <>
          <Select value={format} onValueChange={setFormat} disabled={running}>
            <SelectTrigger size="sm" className="w-[132px]" aria-label="匯出格式">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {formats.map((option) => (
                <SelectItem key={option.id} value={option.id}>
                  {option.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          {running ? (
            <Button
              size="sm"
              variant="outline"
              onClick={() => {
                setWritten(null)
                toast('匯出已停止，沒有儲存檔案。')
              }}
            >
              停止
            </Button>
          ) : (
            <Button size="sm" variant="outline" onClick={() => setWritten(0)}>
              匯出…
            </Button>
          )}
        </>
      }
    >
      {running && (
        <div className="flex flex-col gap-1.5">
          <Progress value={(written / total) * 100} aria-label="匯出進度" className="h-1.5" />
          <span className="text-xs text-muted-foreground tabular">
            已寫入 {num(written)} / 約 {num(total)} 筆記錄 · {((written / total) * 412).toFixed(0)} MB ·{' '}
            {written / total > 0.85 ? '剩不到一分鐘' : `大約還要 ${Math.max(1, Math.round((1 - written / total) * 3))} 分鐘`}
          </span>
        </div>
      )}
    </SettingRow>
  )
}

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
      <Group
        title="連網"
        intro={
          <>
            只有這兩項會連線到網站，其他一切都留在這台電腦上。 <MovedFrom>一般 › 連網</MovedFrom>
          </>
        }
      >
        <SettingRow
          title="連結預覽"
          description="下載每個頁面提供的預覽圖。網站會看到來自你電腦的請求。"
          control={<Switch checked={previews} onCheckedChange={setPreviews} aria-label="連結預覽" />}
        />
        <PreviewImagesRow />
        <SettingRow
          title="頁面摘要"
          description="抓取你看過的頁面的簡短摘要，讓搜尋能依內容找到它們。網站不會拿到你的 Cookie 或帳號。"
          control={<Switch checked={summaries} onCheckedChange={setSummaries} aria-label="頁面摘要" />}
        />
      </Group>
      <Group title="匯出">
        <ExportRow />
      </Group>
    </SettingsSection>
  )
}
