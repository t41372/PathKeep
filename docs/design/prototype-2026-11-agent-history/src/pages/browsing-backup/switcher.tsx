/**
 * Prototype-only: switches this page into each state the real Backup page
 * can be in, so a reviewer can see them without waiting for them to happen.
 * Not part of the design.
 */
import type { ReactNode } from 'react'
import { useSearchParams } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { FIREFOX_PATH, TAKEOUT_PATH } from './import-data'
import type { BrowserPreset, TakeoutPreset } from './import-flows'
import type { Scene, SourcesScene } from './scene'

const sourcesOptions: [SourcesScene, string][] = [
  ['normal', 'Safari 需要授權、Firefox 關著、2 個沒有紀錄'],
  ['fullDiskAccess', 'Safari 缺完整磁碟取用權限（說明列）'],
  ['discoveryError', '掃描出錯（說明列）'],
  ['empty', '一個瀏覽器都沒有'],
  ['statsLoading', '瀏覽次數載入中'],
  ['statsFailed', '瀏覽次數讀不到'],
]

const takeoutOptions: [TakeoutPreset, string][] = [
  ['idle', '還沒選檔案'],
  ['error', '讀不了這個檔案'],
  ['inspecting', '正在讀取'],
  ['found', '找到記錄（有略過的、無法辨識的檔案）'],
  ['nothing', '找不到瀏覽記錄'],
  ['myActivity', '「我的活動」是 HTML'],
  ['importing', '匯入中'],
  ['stopping', '正在停止'],
  ['failed', '匯入出錯'],
  ['done', '匯入完成'],
  ['continued', '完成，接著上次中斷的'],
  ['stopped', '中途停止'],
]

const browserOptions: [BrowserPreset, string][] = [
  ['idle', '還沒選檔案'],
  ['error', '讀不了這個檔案'],
  ['inspecting', '正在讀取'],
  ['preview', '預覽（Firefox，含同步副本）'],
  ['nothingNew', '預覽：都已經在存檔裡'],
  ['importing', '匯入中'],
  ['stopping', '正在停止'],
  ['failed', '匯入出錯'],
  ['done', '匯入完成'],
  ['stopped', '中途停止'],
]

function Pick<T extends string>({ label, value, options, onChange }: { label: string; value: T; options: [T, string][]; onChange: (v: T) => void }) {
  return (
    <label className="flex items-center justify-between gap-2">
      <span className="shrink-0">{label}</span>
      <Select value={value} onValueChange={(v) => onChange(v as T)}>
        <SelectTrigger size="sm" className="w-64 bg-card text-xs" aria-label={`原型：${label}`}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {options.map(([id, text]) => (
            <SelectItem key={id} value={id} className="text-xs">
              {text}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </label>
  )
}

function Toggle({ label, checked, onChange }: { label: ReactNode; checked: boolean; onChange: (on: boolean) => void }) {
  return (
    <label className="flex items-center justify-between gap-2">
      <span>{label}</span>
      <Switch checked={checked} onCheckedChange={onChange} />
    </label>
  )
}

export function Switcher({
  scene,
  onScene,
  onRunning,
  takeout,
  onTakeout,
  browser,
  onBrowser,
  onDrop,
}: {
  scene: Scene
  onScene: (patch: Partial<Scene>) => void
  onRunning: (on: boolean) => void
  takeout: TakeoutPreset
  onTakeout: (preset: TakeoutPreset) => void
  browser: BrowserPreset
  onBrowser: (preset: BrowserPreset) => void
  onDrop: (path: string) => void
}) {
  const [, setParams] = useSearchParams()
  return (
    <section aria-label="原型：狀態切換" className="flex flex-col gap-2.5 rounded-xl border border-dashed border-foreground/30 px-4 py-3 text-xs text-muted-foreground">
      <span>原型：切換這一頁的狀態（不是設計的一部分）。頁面本身照今天的備份頁。</span>
      <div className="grid grid-cols-1 gap-x-8 gap-y-2 lg:grid-cols-2">
        <Toggle label="正在備份" checked={scene.running} onChange={onRunning} />
        <Pick label="來源" value={scene.sources} options={sourcesOptions} onChange={(sources) => onScene({ sources })} />
        <Pick label="Takeout 匯入" value={takeout} options={takeoutOptions} onChange={onTakeout} />
        <Pick label="瀏覽器歷史匯入" value={browser} options={browserOptions} onChange={onBrowser} />
        <Toggle label="檔案正拖過視窗" checked={scene.hovering} onChange={(hovering) => onScene({ hovering })} />
        <Toggle label="沒有原生檔案選擇器（瀏覽器裡：輸入路徑）" checked={!scene.nativeFiles} onChange={(off) => onScene({ nativeFiles: !off })} />
        <Toggle label="最近的匯入：一筆都沒有" checked={scene.importsEmpty} onChange={(importsEmpty) => onScene({ importsEmpty })} />
        <Toggle label="最近記錄：一筆都沒有" checked={scene.runsEmpty} onChange={(runsEmpty) => onScene({ runsEmpty })} />
        <div className="flex items-center justify-between gap-2">
          <span>把檔案拖進視窗</span>
          <span className="flex gap-1.5">
            <Button size="sm" variant="outline" className="h-7 text-xs" onClick={() => onDrop(TAKEOUT_PATH)}>
              .zip
            </Button>
            <Button size="sm" variant="outline" className="h-7 text-xs" onClick={() => onDrop(FIREFOX_PATH)}>
              places.sqlite
            </Button>
          </span>
        </div>
        <div className="flex items-center justify-between gap-2">
          <span>執行詳情</span>
          <Button size="sm" variant="outline" className="h-7 text-xs" onClick={() => setParams({ run: '416' })}>
            打開有警告的那次
          </Button>
        </div>
      </div>
    </section>
  )
}
