/**
 * The 背景任務 page's 瀏覽紀錄 section: everything in today's Settings ›
 * Background work that is status (`src/features/settings/background/`), in
 * the order it has there: the two queues, page summaries, link previews,
 * then 搜尋與洞察資料. All of it is about browsing history today.
 */
import { SettingsGroup } from '@/components/app/setting-row'
import { Switch } from '@/components/ui/switch'
import { kindById } from '~/kinds'
import { SpaceTile } from '~/shell/space-rail'
import { followQueuePause, setRebuildRunning } from './jobs'
import { setBackground, useBackground, type BackgroundState } from './background-store'
import { InsightsDataRow, PageDetailsRow } from './derived-data-rows'
import { LinkPreviewsRow, PageSummariesRow } from './online-work-rows'
import { AiQueueRow, InsightsQueueRow } from './queue-rows'

function ProtoToggle({ label, field, onChange }: { label: string; field: keyof BackgroundState; onChange?: (on: boolean) => void }) {
  const state = useBackground()
  const on = Boolean(state[field])
  return (
    <label className="flex items-center gap-2">
      {label}
      <Switch
        checked={on}
        onCheckedChange={(next) => {
          setBackground({ [field]: next })
          onChange?.(next)
        }}
        aria-label={`原型：${label}`}
      />
    </label>
  )
}

/** Prototype only: the states these rows switch on. */
function Switcher() {
  const { aiOn, summariesOn, previewsOn } = useBackground()
  return (
    <div className="flex flex-wrap items-center gap-x-5 gap-y-2 rounded-lg border border-dashed px-3 py-2 text-xs text-muted-foreground">
      <span className="font-medium">原型：</span>
      <ProtoToggle label="背景工作暫停" field="paused" onChange={followQueuePause} />
      <label className="flex items-center gap-2">
        依意思搜尋與助理關閉
        <Switch checked={!aiOn} onCheckedChange={(off) => setBackground({ aiOn: !off })} aria-label="原型：依意思搜尋與助理關閉" />
      </label>
      <label className="flex items-center gap-2">
        網頁摘要關閉
        <Switch checked={!summariesOn} onCheckedChange={(off) => setBackground({ summariesOn: !off })} aria-label="原型：網頁摘要關閉" />
      </label>
      <label className="flex items-center gap-2">
        連結預覽關閉
        <Switch checked={!previewsOn} onCheckedChange={(off) => setBackground({ previewsOn: !off })} aria-label="原型：連結預覽關閉" />
      </label>
      <ProtoToggle label="洞察正在重建" field="protoRebuilding" onChange={setRebuildRunning} />
      <ProtoToggle label="佇列是空的" field="protoEmpty" />
      <ProtoToggle label="讀取中" field="protoLoading" />
    </div>
  )
}

export function BrowsingWork() {
  const kind = kindById.browsing
  return (
    <section className="flex flex-col gap-3">
      <h2 className="flex items-center gap-2 text-[15px] font-semibold">
        <SpaceTile kind={kind} size="sm" />
        {kind.label}
      </h2>
      <Switcher />
      <InsightsQueueRow />
      <AiQueueRow />
      <PageSummariesRow />
      <LinkPreviewsRow />
      <SettingsGroup title="搜尋與洞察資料" note="這些資料是從你的歷史紀錄中計算出來的，隨時可以清除並重新建立，你的歷史紀錄不會被改動。" />
      <InsightsDataRow />
      <PageDetailsRow />
    </section>
  )
}
