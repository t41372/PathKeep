/**
 * Prototype only, not part of the design: picks which scheduler state the
 * 自動備份 card shows, so a reviewer can see each one the real card switches on.
 */
import { cn } from '@/lib/cn'
import { pickFixture, useSchedule, type AttemptsCase, type InstallState, type PermissionCause, type Platform } from './schedule-store'

function Chips<T extends string | number>({ label, value, options, onPick }: { label: string; value: T; options: [T, string][]; onPick: (v: T) => void }) {
  return (
    <div data-proto={label} className="flex flex-wrap items-center gap-1.5">
      <span className="w-[84px] shrink-0">{label}</span>
      {options.map(([v, text]) => (
        <button
          key={String(v)}
          type="button"
          aria-pressed={value === v}
          onClick={() => onPick(v)}
          className={cn('rounded-md border px-2 py-0.5 transition-colors hover:bg-muted', value === v && 'border-foreground/30 bg-muted text-foreground')}
        >
          {text}
        </button>
      ))}
    </div>
  )
}

const states: Record<Platform, [InstallState, string][]> = {
  macos: [
    ['installed', '已安裝'],
    ['not-installed', '未安裝'],
    ['mismatch', '不一致'],
    ['permission-warning', '需要授權'],
    ['legacy-install-detected', '舊版工作'],
  ],
  windows: [
    ['installed', '已安裝'],
    ['not-installed', '未安裝'],
    ['mismatch', '不一致'],
    ['permission-warning', '需要授權'],
  ],
  linux: [['manual-review', '手動設定（無法安裝的平台）']],
}

const causes: Record<Platform, [PermissionCause, string][]> = {
  macos: [
    ['not-loaded', '沒有載入'],
    ['unreadable', '讀不到檔案'],
    ['missing-loaded', '檔案不見了'],
  ],
  windows: [
    ['access-denied', '拒絕存取'],
    ['inspection-failed', '查詢失敗'],
  ],
  linux: [],
}

const attempts: [AttemptsCase, string][] = [
  ['normal', '正常'],
  ['running', '正在執行'],
  ['failed', '上次失敗'],
  ['partial', '未完成'],
  ['interrupted', '中途停止'],
  ['overdue', '已經停了'],
  ['unreadable', '記錄讀不到'],
]

export function ScheduleSwitcher() {
  const f = useSchedule()
  return (
    <div className="flex flex-col gap-2 rounded-lg border border-dashed px-3 py-2.5 text-xs text-muted-foreground">
      <span className="font-medium">原型：自動備份卡的狀態</span>
      <Chips<Platform>
        label="平台"
        value={f.platform}
        options={[
          ['macos', 'macOS'],
          ['windows', 'Windows'],
          ['linux', 'Linux'],
        ]}
        onPick={(platform) => pickFixture({ platform })}
      />
      <Chips<InstallState> label="排程器" value={f.state} options={states[f.platform]} onPick={(state) => pickFixture({ state })} />
      {f.state === 'permission-warning' && <Chips<PermissionCause> label="需要授權的原因" value={f.cause} options={causes[f.platform]} onPick={(cause) => pickFixture({ cause })} />}
      <Chips<number>
        label="間隔"
        value={f.hours}
        options={[
          [1, '每小時'],
          [6, '每 6 小時'],
          [24, '每天'],
          [1.5, '自訂：90 分鐘'],
          [48, '自訂：2 天'],
        ]}
        onPick={(hours) => pickFixture({ hours })}
      />
      <Chips<AttemptsCase> label="自動執行" value={f.attempts} options={attempts} onPick={(a) => pickFixture({ attempts: a })} />
    </div>
  )
}
