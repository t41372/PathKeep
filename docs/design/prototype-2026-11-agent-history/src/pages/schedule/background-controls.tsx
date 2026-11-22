/**
 * The rows of today's Settings › Background work that are choices, not
 * readouts: run / pause and jobs at once (`QueueControls` in
 * `src/features/settings/background/queue-rows.tsx`), and link previews'
 * 何時抓取 and 保留圖片 (`LinkPreviewsRow` in `online-work-rows.tsx`, whose
 * coverage and 立即清理 moved to the 背景任務 page).
 */
import { toast } from 'sonner'
import { SettingRow } from '@/components/app/setting-row'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { keepLabels, modeLabels, setBackground, useBackground, type KeepChoice, type PreviewMode } from '../tasks/background-store'
import { followQueuePause } from '../tasks/jobs'

const CONCURRENCY_CHOICES = [1, 2, 3, 4]

/** `src/features/settings/row-select.tsx`. */
function RowSelect({ id, value, options, onChange }: { id?: string; value: string; options: { value: string; label: string }[]; onChange: (v: string) => void }) {
  return (
    <Select value={value} onValueChange={onChange}>
      <SelectTrigger id={id} size="sm" className="min-w-[120px] bg-card text-[13px]">
        <SelectValue />
      </SelectTrigger>
      <SelectContent align="end">
        {options.map((o) => (
          <SelectItem key={o.value} value={o.value}>
            {o.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}

export function QueueControls() {
  const { paused, concurrency } = useBackground()
  const current = Math.max(1, concurrency)
  const choices = CONCURRENCY_CHOICES.includes(current) ? CONCURRENCY_CHOICES : [...CONCURRENCY_CHOICES, current].sort((a, b) => a - b)
  return (
    <>
      <SettingRow
        title="執行背景工作"
        description={paused ? '已暫停。新工作會在這裡等候，直到你重新開啟。備份照常進行。' : '更新會自動進行。'}
        htmlFor="settings-background-run"
        control={
          <Switch
            id="settings-background-run"
            checked={!paused}
            onCheckedChange={(on) => {
              setBackground({ paused: !on })
              followQueuePause(!on)
              toast.success(on ? '背景工作已恢復。' : '背景工作已暫停。')
            }}
          />
        }
      />
      <SettingRow
        title="同時執行的工作數"
        description="數量越多完成越快，但會占用更多電腦資源。修改後對之後開始的工作生效。"
        htmlFor="settings-background-concurrency"
        control={
          <RowSelect
            id="settings-background-concurrency"
            value={String(current)}
            onChange={(v) => setBackground({ concurrency: Number(v) })}
            options={choices.map((n) => ({ value: String(n), label: String(n) }))}
          />
        }
      />
    </>
  )
}

const keepChoices: Exclude<KeepChoice, 'custom'>[] = ['off', 'days30', 'days90', 'size100', 'size500']

/**
 * The two link-preview choices (when to fetch, which images to keep). Today
 * they sit in Settings › Background work under a row that says where the
 * switch is; here they sit under the 連結預覽 switch itself, in Settings ›
 * 瀏覽紀錄, and show only while it is on.
 */
export function LinkPreviewSelects() {
  const { previewMode, keep } = useBackground()
  return (
    <div className="flex flex-col gap-2.5 border-t pt-3 text-[13px]">
      <div className="flex items-center justify-between gap-4">
        <label htmlFor="settings-previews-mode">何時抓取</label>
        <RowSelect
          id="settings-previews-mode"
          value={previewMode}
          onChange={(v) => setBackground({ previewMode: v as PreviewMode })}
          options={(['on_demand', 'background'] as const).map((m) => ({ value: m, label: modeLabels[m] }))}
        />
      </div>
      <div className="flex items-center justify-between gap-4">
        <label htmlFor="settings-previews-keep">保留圖片</label>
        <RowSelect
          id="settings-previews-keep"
          value={keep}
          onChange={(v) => setBackground({ keep: v as KeepChoice })}
          options={[...(keep === 'custom' ? (['custom'] as const) : []), ...keepChoices].map((c) => ({ value: c, label: keepLabels[c] }))}
        />
      </div>
    </div>
  )
}
