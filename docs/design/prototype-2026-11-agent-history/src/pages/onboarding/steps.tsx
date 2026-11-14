/**
 * Onboarding step bodies. The new part is "要保存什麼": the data types come
 * from the registry, each with why it needs saving; planned ones are shown
 * but can't be picked. The sources step then lists what was found for each
 * chosen type.
 */
import { Check, FolderGit2, HardDrive, ShieldCheck, TriangleAlert } from 'lucide-react'
import type { ReactNode } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { ChoiceCard, ChoiceGroup, RadioMark } from './choice'
import { cn } from '@/lib/cn'
import { HarnessBadge } from '~/components/bits'
import { bytes, num } from '~/data/format'
import type { HarnessId } from '~/data/types'
import { kinds, type KindId } from '~/kinds'
import { AGENT_ARCHIVE_RATIO, archiveBytes, browserSources, harnessSources, sourceBytesOfKind } from '../backup/sources-data'

export function StepHeading({ title, body }: { title: string; body?: ReactNode }) {
  return (
    <header className="flex flex-col gap-2">
      <h1 data-step-heading tabIndex={-1} className="text-[28px] leading-tight font-semibold tracking-[-0.02em] outline-none">
        {title}
      </h1>
      {body && <p className="text-[14px] leading-relaxed text-muted-foreground">{body}</p>}
    </header>
  )
}

const retention = [
  { name: 'Claude Code', keep: '預設 30 天', width: '4%' },
  { name: 'Chrome', keep: '90 天', width: '12%' },
  { name: 'Slack 免費版', keep: '90 天', width: '12%' },
  { name: 'Safari', keep: '預設 1 年', width: '40%' },
  { name: 'Codex', keep: '不自動刪，但常被手動清掉', width: '64%' },
]

export function WelcomeStep() {
  return (
    <>
      <StepHeading title="留住會被自動刪掉的東西" body="瀏覽紀錄、跟 Agent 的對話，很多工具過一陣子就自己清掉了。PathKeep 定時把它們複製進你電腦上的存檔，之後隨時能找、能看。" />
      <section className="flex flex-col gap-3.5 rounded-xl border bg-card p-5 shadow-card">
        <h2 className="text-[13px] font-normal text-muted-foreground">各家自己會留多久</h2>
        <ul className="flex flex-col gap-3">
          {retention.map((row) => (
            <li key={row.name} className="flex items-center gap-3">
              <span className="w-24 shrink-0 text-[13px]">{row.name}</span>
              <span className="relative h-1.5 flex-1 overflow-hidden rounded-full bg-muted">
                <span className="absolute inset-y-0 left-0 rounded-full bg-brand" style={{ width: row.width }} />
              </span>
              <span className="w-[168px] shrink-0 text-right text-xs text-muted-foreground">{row.keep}</span>
            </li>
          ))}
        </ul>
      </section>
      <ul className="flex flex-col gap-2.5 text-[13px]">
        {[
          [HardDrive, '全部存在這台電腦，不上傳到任何地方'],
          [ShieldCheck, '只讀來源，不改、不刪任何檔案'],
          [Check, '存下的東西都能搜尋，沒有筆數上限'],
        ].map(([Icon, text]) => {
          const I = Icon as typeof HardDrive
          return (
            <li key={text as string} className="flex items-center gap-2.5">
              <I className="size-4 text-muted-foreground" />
              {text as string}
            </li>
          )
        })}
      </ul>
    </>
  )
}

export function KindsStep({ selected, onToggle }: { selected: Set<KindId>; onToggle: (id: KindId) => void }) {
  return (
    <>
      <StepHeading title="要保存什麼？" body="每一種之後都能在設定裡加減。關掉不會刪掉已經存下的資料。" />
      <div className="grid grid-cols-2 gap-3 max-[760px]:grid-cols-1">
        {kinds.map((kind) => {
          const on = selected.has(kind.id)
          const Icon = kind.icon
          return (
            <button
              key={kind.id}
              type="button"
              role="checkbox"
              aria-checked={on}
              disabled={!kind.real}
              onClick={() => onToggle(kind.id)}
              className={cn(
                'group flex flex-col gap-2 rounded-xl border-[1.5px] bg-card p-4 text-left shadow-card transition-[border-color,box-shadow] duration-150 outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50',
                kind.real ? 'hover:border-foreground/30' : 'cursor-not-allowed bg-card/40 shadow-none',
                on && 'border-foreground',
              )}
            >
              <span className="flex items-center gap-2.5">
                <span className={cn('flex size-8 items-center justify-center rounded-lg bg-muted', !kind.real && 'opacity-60')}>
                  <Icon className="size-4" strokeWidth={1.75} />
                </span>
                <span className={cn('flex-1 font-medium', !kind.real && 'text-muted-foreground')}>{kind.label}</span>
                {kind.real ? (
                  <span className={cn('flex size-[18px] items-center justify-center rounded-[5px] border-[1.5px] border-border transition-colors', on && 'border-foreground bg-foreground text-background')}>
                    {on && <Check className="size-3" strokeWidth={3} />}
                  </span>
                ) : (
                  <Badge variant="outline" className="font-normal text-muted-foreground">
                    規劃中
                  </Badge>
                )}
              </span>
              <span className="text-xs leading-relaxed text-muted-foreground">{kind.why}</span>
              <span className="text-[11.5px] text-muted-foreground/80">{kind.sources.join('、')}</span>
            </button>
          )
        })}
      </div>
      {selected.size === 0 && <p className="text-[13px] text-destructive">至少選一種。</p>}
    </>
  )
}

export function SourcesStep({
  kindsOn,
  harnessOn,
  onHarness,
  browserOn,
  onBrowser,
}: {
  kindsOn: Set<KindId>
  harnessOn: Set<HarnessId>
  onHarness: (id: HarnessId) => void
  browserOn: Set<string>
  onBrowser: (key: string) => void
}) {
  const claude = harnessSources.find((h) => h.autoDelete)
  return (
    <>
      <StepHeading title="找到這些來源" body="PathKeep 只讀這些資料夾，不改、不刪任何檔案。不想備份的可以取消勾選。" />
      {kindsOn.has('browsing') && (
        <section className="overflow-hidden rounded-xl border bg-card shadow-card">
          <h2 className="px-5 py-3 text-sm font-semibold">瀏覽紀錄</h2>
          <ul>
            {browserSources.map((b) => {
              const key = `${b.name}-${b.profile}`
              return (
                <li key={key} className="border-t">
                  <label className="flex cursor-pointer items-center gap-3.5 px-5 py-2.5 hover:bg-muted/40">
                    <Checkbox checked={browserOn.has(key)} onCheckedChange={() => onBrowser(key)} />
                    <img src={b.icon} alt="" className="size-6" />
                    <span className="flex min-w-0 flex-1 flex-col">
                      <span className="text-sm font-medium">{b.name}</span>
                      <span className="text-xs text-muted-foreground">{b.profile}</span>
                    </span>
                    <span className="font-mono text-[13px] text-muted-foreground tabular">{num(b.visits)} 筆</span>
                  </label>
                </li>
              )
            })}
          </ul>
        </section>
      )}
      {kindsOn.has('agent') && (
        <section className="overflow-hidden rounded-xl border bg-card shadow-card">
          <h2 className="px-5 py-3 text-sm font-semibold">Agent 對話</h2>
          <ul>
            {harnessSources.map((h) => (
              <li key={h.id} className="border-t">
                <label className="flex cursor-pointer items-center gap-3.5 px-5 py-2.5 hover:bg-muted/40">
                  <Checkbox checked={harnessOn.has(h.id)} onCheckedChange={() => onHarness(h.id)} />
                  <HarnessBadge id={h.id} className="w-6 justify-center" />
                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="text-sm font-medium">{h.name}</span>
                    <span className="truncate font-mono text-[11.5px] text-muted-foreground">{h.root}</span>
                  </span>
                  <span className="flex flex-col items-end text-right">
                    <span className="font-mono text-[13px] text-muted-foreground tabular">{num(h.chats)} 段對話</span>
                    <span className="text-xs text-muted-foreground">{h.subagents ? `另有 ${num(h.subagents)} 個 subagent · ` : ''}{bytes(h.sourceBytes)}</span>
                  </span>
                </label>
              </li>
            ))}
          </ul>
          <div className="flex items-start gap-3 border-t bg-muted/30 px-5 py-3.5 text-[13px]">
            <FolderGit2 className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
            <span className="leading-relaxed text-muted-foreground">
              備份時也會讀每個專案資料夾的 <code className="font-mono text-[12px]">.git</code>，只看 repo 根目錄、worktree 屬於哪個 repo、遠端網址，用來把同一個專案的資料夾併在一起。不讀程式碼。之後可以在 Agent 對話的備份頁關掉。
            </span>
          </div>
          {claude && harnessOn.has(claude.id) && (
            <div className="flex items-start gap-3 border-t px-5 py-3.5 text-[13px]">
              <TriangleAlert className="mt-0.5 size-4 shrink-0 text-amber-600 dark:text-amber-300" />
              <span className="leading-relaxed text-muted-foreground">
                {claude.name} 設定成 {claude.autoDelete!.days} 天後刪掉舊對話。第一次備份會把現在還在的 {num(claude.chats - claude.gone)} 段先存下來。
              </span>
            </div>
          )}
        </section>
      )}
    </>
  )
}

export function StorageStep({ kindsOn }: { kindsOn: Set<KindId> }) {
  const chosen = kinds.filter((k) => kindsOn.has(k.id) && k.real)
  const total = chosen.reduce((s, k) => s + (archiveBytes[k.id] ?? 0), 0)
  return (
    <>
      <StepHeading title="存在哪裡" body="存檔放在 PathKeep 自己的資料夾，所有資料類型都在這一個存檔裡。" />
      <div className="flex items-center gap-3 rounded-xl border bg-card px-5 py-4 shadow-card">
        <HardDrive className="size-5 text-muted-foreground" />
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="truncate font-mono text-[13px]">~/Library/Application Support/PathKeep/archive</span>
          <span className="text-xs text-muted-foreground">Macintosh HD · 還有 312 GB 可用</span>
        </span>
        <Button variant="outline" size="sm">
          在 Finder 顯示
        </Button>
      </div>
      <section className="flex flex-col gap-3 rounded-xl border bg-card p-5 shadow-card">
        <h2 className="text-[13px] text-muted-foreground">第一次備份後大約會用掉</h2>
        {chosen.map((k) => (
          <div key={k.id} className="flex items-baseline justify-between gap-3 text-[13px]">
            <span className="flex items-center gap-1.5">
              <k.icon className="size-3.5 text-muted-foreground" />
              {k.label}
            </span>
            <span className="text-right">
              <span className="font-mono tabular">{bytes(archiveBytes[k.id] ?? 0)}</span>
              <span className="block text-xs text-muted-foreground">
                來源 {bytes(sourceBytesOfKind[k.id] ?? 0)}
                {k.id === 'agent' && `，存檔約是它的 ${AGENT_ARCHIVE_RATIO} 倍（原始紀錄加搜尋索引）`}
              </span>
            </span>
          </div>
        ))}
        <div className="flex justify-between border-t pt-3 text-[13px] font-medium">
          <span>合計</span>
          <span className="font-mono tabular">{bytes(total)}</span>
        </div>
      </section>
    </>
  )
}

export type Encryption = 'password' | 'none'

export function EncryptionStep({ value, onChange }: { value: Encryption; onChange: (v: Encryption) => void }) {
  return (
    <>
      <StepHeading title="要不要加密？" body="所有資料類型放在同一個存檔，這裡選的套用到全部。" />
      <ChoiceGroup value={value} onValueChange={onChange} label="存檔加密">
        <ChoiceCard
          value="password"
          title="用密碼加密（建議）"
          body="存檔用 SQLCipher 加密，開啟 PathKeep 要輸入密碼或用 Touch ID。忘了密碼就打不開。"
          trailing={<RadioMark />}
        />
        <ChoiceCard
          value="none"
          title="不加密"
          body="任何能讀你檔案的程式都能讀存檔。Agent 對話裡常有程式碼、金鑰和私人路徑，請想清楚。"
          trailing={<RadioMark />}
        />
      </ChoiceGroup>
      {value === 'password' && (
        <div className="flex flex-col gap-2.5">
          <Input type="password" placeholder="密碼" aria-label="密碼" />
          <Input type="password" placeholder="再輸入一次" aria-label="再輸入一次密碼" />
        </div>
      )}
    </>
  )
}

export type Schedule = '1h' | '6h' | '1d' | 'manual'

export function ScheduleStep({ value, onChange }: { value: Schedule; onChange: (v: Schedule) => void }) {
  const options: { id: Schedule; title: string; body: string }[] = [
    { id: '1h', title: '每小時（建議）', body: '最不容易漏掉工具自己刪掉的東西。' },
    { id: '6h', title: '每 6 小時', body: '一天備份四次。' },
    { id: '1d', title: '每天', body: '一天備份一次。' },
    { id: 'manual', title: '手動', body: '只在你按「立即備份」時備份。工具自己刪掉的東西，PathKeep 可能來不及存。' },
  ]
  return (
    <>
      <StepHeading title="多久備份一次？" body="一個排程，所有資料類型一起備份。" />
      <ChoiceGroup value={value} onValueChange={onChange} label="備份排程">
        {options.map((o) => (
          <ChoiceCard key={o.id} value={o.id} title={o.title} body={o.body} leading={<RadioMark />} />
        ))}
      </ChoiceGroup>
    </>
  )
}

export type AiChoice = 'none' | 'local' | 'service'

/** Unchanged from today's onboarding; shown so the step list is the real one. */
export function AiStep({ value, onChange }: { value: AiChoice; onChange: (v: AiChoice) => void }) {
  const options: { id: AiChoice; title: string; body: string }[] = [
    { id: 'none', title: '先不要', body: '備份、瀏覽、搜尋、洞察都不需要 AI。之後可以在設定裡開。' },
    { id: 'local', title: '本機語意搜尋', body: '下載一個小模型，讓搜尋也能找意思相近的內容。全部在這台電腦上跑。' },
    { id: 'service', title: '連接 AI 服務', body: '用你自己的服務（Ollama、LM Studio、OpenAI 或相容的位址）來問問題。' },
  ]
  return (
    <>
      <StepHeading title="要不要用 AI？" body="這些都是可選的。這一步跟現在的一樣，沒有改。" />
      <ChoiceGroup value={value} onValueChange={onChange} label="AI">
        {options.map((o) => (
          <ChoiceCard key={o.id} value={o.id} title={o.title} body={o.body} leading={<RadioMark />} />
        ))}
      </ChoiceGroup>
    </>
  )
}

export function DoneStep({ summary }: { summary: { label: string; value: ReactNode }[] }) {
  return (
    <>
      <StepHeading title="都設好了" body="按下開始，PathKeep 會做第一次備份。之後照排程在背景跑。" />
      <dl className="flex flex-col rounded-xl border bg-card px-5 py-2 shadow-card">
        {summary.map((row) => (
          <div key={row.label} className="flex items-baseline justify-between gap-6 border-t py-3 text-[13px] first:border-t-0">
            <dt className="text-muted-foreground">{row.label}</dt>
            <dd className="text-right">{row.value}</dd>
          </div>
        ))}
      </dl>
    </>
  )
}
