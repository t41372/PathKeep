/**
 * The browsing space's Backup page (`#/browsing/backup`): today's Backup page
 * (`src/features/backup/backup-page.tsx`) with fake data, and the three
 * changes data-spaces.md §4 makes to it:
 * 1. The 「自動備份」 card moves to Settings › 排程與背景工作; one row at the
 *    foot of the page says the schedule and links there (`schedule-row.tsx`).
 * 2. 最近記錄 lists only what each run did for browsing history (`runs-data.ts`).
 * 3. 「立即備份」 backs up only this type and says so in its toast.
 * Everything else, every card and state, is today's.
 */
import { RefreshCw } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import { PageHeader, PageScroll } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import { useSearchParams } from 'react-router-dom'
import { cn } from '@/lib/cn'
import { NOW } from '~/data/fixture'
import { BrowserImportCard } from './browser-import-card'
import { dropTarget } from './file-drop'
import { number } from './format'
import { ImportCard } from './import-card'
import { runningBatch } from './import-data'
import { useBrowserImport, useTakeoutImport, type BrowserPreset, type TakeoutPreset } from './import-flows'
import { ImportsCard } from './imports-card'
import { ProgressCard } from './progress-card'
import { RunsCard } from './runs-card'
import { finishedRun } from './runs-data'
import { initialScene, SceneContext, type BackupProgress, type Scene } from './scene'
import { ScheduleRow } from './schedule-row'
import { SourcesCard } from './sources-card'
import { Switcher } from './switcher'

/** The sources a browsing backup reads, in order, with the visits each has to read. */
const backupSteps: [string, number][] = [
  ['Google Chrome 個人', 412_308],
  ['Google Chrome 工作', 96_115],
  ['Google Chrome 學校', 8_904],
  ['Arc Default', 51_447],
  ['Microsoft Edge Default', 23_016],
  ['Microsoft Edge Profile 1', 4_210],
  ['Microsoft Edge Profile 2', 1_302],
  ['Microsoft Edge Profile 3', 0],
]

const progressAt = (step: number, fraction: number): BackupProgress => ({
  completedProfiles: step,
  totalProfiles: backupSteps.length,
  sourceLabel: backupSteps[step][0],
  processedRecords: Math.round(backupSteps[step][1] * fraction),
  totalRecords: backupSteps[step][1],
  progressPercent: ((step + fraction) / backupSteps.length) * 100,
})

/** Stand-in for `useBackupRunner().run()`: 「正在準備…」, then source by source. */
function useFakeBackup(setScene: (update: (s: Scene) => Scene) => void, openRun: (id: number) => void) {
  const timers = useRef<number[]>([])
  const clear = () => {
    timers.current.forEach((id) => window.clearTimeout(id))
    timers.current = []
  }
  useEffect(() => clear, [])
  const later = (ms: number, fn: () => void) => timers.current.push(window.setTimeout(fn, ms))
  const set = (patch: Partial<Scene>) => setScene((s) => ({ ...s, ...patch }))

  return {
    start() {
      clear()
      set({ running: true, progress: null, finished: false })
      backupSteps.forEach((_, step) => {
        later(900 + step * 1400, () => set({ progress: progressAt(step, 0.3) }))
        later(900 + step * 1400 + 700, () => set({ progress: progressAt(step, 0.8) }))
      })
      later(900 + backupSteps.length * 1400, () => {
        set({ running: false, progress: null, finished: true })
        // The shell's toast when a backup ends (`app/backup-runner.tsx`): partial
        // because Safari was skipped, with its warning count and a link to the run.
        toast.warning(`備份未完成，已儲存 ${number(finishedRun.newVisits)} 筆新記錄`, {
          description: '1 則警告：有內容被略過或沒有完成。',
          duration: 10_000,
          action: { label: '詳情', onClick: () => openRun(finishedRun.id) },
        })
      })
    },
    /** The switcher holds it midway, on the second source. */
    hold(on: boolean) {
      clear()
      set(on ? { running: true, progress: progressAt(1, 0.42) } : { running: false, progress: null })
    },
  }
}

/** `BackupNowButton`: 「備份中… 42%」 and disabled while any backup runs. */
function BackupNowButton({ scene, onStart }: { scene: Scene; onStart: () => void }) {
  const percent = scene.progress?.progressPercent
  return (
    <Button
      className="h-9"
      disabled={scene.running}
      onClick={() => {
        // Design change: only this type, and the toast says where backing up everything lives.
        toast.success('開始備份「瀏覽紀錄」', {
          description: '只備份這個類型。要全部一起備份，用左下角背景任務選單裡的「全部立即備份」。',
        })
        onStart()
      }}
    >
      <RefreshCw className={cn(scene.running && 'animate-spin')} />
      {scene.running ? (percent != null ? `備份中… ${Math.round(percent)}%` : '備份中…') : '立即備份'}
    </Button>
  )
}

export default function BrowsingBackupPage() {
  const [scene, setScene] = useState<Scene>(initialScene)
  const [takeoutPreset, setTakeoutPreset] = useState<TakeoutPreset>('idle')
  const [browserPreset, setBrowserPreset] = useState<BrowserPreset>('idle')
  const takeout = useTakeoutImport()
  const browser = useBrowserImport()
  const [, setParams] = useSearchParams()
  const backup = useFakeBackup(setScene, (id) => setParams({ run: String(id) }))
  const importing =
    takeout.state.step === 'importing'
      ? runningBatch('takeout', takeout.state.path)
      : browser.state.step === 'importing'
        ? runningBatch('browser-file', browser.state.path)
        : null
  // Hourly, on the hour: the next run is at the top of the coming hour.
  const nextRun = NOW + 60 * 60_000 - ((NOW / 60_000) % 60) * 60_000
  // `useWindowFileDrop`: a dropped file goes to exactly one card, by its name,
  // and only while that card is idle and no backup runs.
  const drop = (path: string) => {
    if (scene.running) return
    if (dropTarget(path) === 'browser') {
      if (browser.state.step === 'idle') browser.inspect(path, 'detect')
    } else if (takeout.state.step === 'idle') takeout.inspect(path)
  }

  return (
    <SceneContext.Provider value={scene}>
      <PageScroll>
        <PageHeader
          title="備份"
          subtitle="PathKeep 從不直接讀取瀏覽器的資料庫，而是先複製一份，再增量追加新內容。"
          actions={<BackupNowButton scene={scene} onStart={backup.start} />}
        />
        <Switcher
          scene={scene}
          onScene={(patch) => setScene((s) => ({ ...s, ...patch }))}
          onRunning={backup.hold}
          takeout={takeoutPreset}
          onTakeout={(preset) => {
            setTakeoutPreset(preset)
            takeout.show(preset)
          }}
          onDrop={drop}
          browser={browserPreset}
          onBrowser={(preset) => {
            setBrowserPreset(preset)
            browser.show(preset)
          }}
        />
        <ScheduleRow on={scene.scheduleOn} next={nextRun} />
        <ProgressCard />
        <SourcesCard />
        <div className="grid grid-cols-1 gap-5 lg:grid-cols-2">
          <ImportCard takeout={takeout} />
          <BrowserImportCard browser={browser} />
        </div>
        {/* Without the 自動備份 card the grid would leave a hole, so this list takes the full row. */}
        <ImportsCard running={importing} />
        <RunsCard />
      </PageScroll>
    </SceneContext.Provider>
  )
}
