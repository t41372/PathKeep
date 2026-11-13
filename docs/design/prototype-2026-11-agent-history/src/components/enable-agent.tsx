/**
 * Turning on Agent history for an existing install: Preview → Manual →
 * Execute. Nothing is read before the person opens the preview, and the
 * preview itself only counts files (number, size, dates), never opens a
 * conversation. Opened by `?enable=agent`.
 */
import { Check, CircleCheck, FolderSearch, GitBranch, Lock, ShieldCheck, X } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Progress } from '@/components/ui/progress'
import { Switch } from '@/components/ui/switch'
import { cn } from '@/lib/cn'
import { archive, harnesses, harnessOrder } from '~/data/archive'
import { bytes, date, num } from '~/data/format'
import type { HarnessId } from '~/data/types'
import { useProto } from '~/proto-state'
import { spaceKeyLabel } from '~/shell/space-key'
import { useSwitchSpace } from '~/shell/space-rail'
import { HarnessBadge } from './bits'

interface Found {
  id: HarnessId
  chats: number
  subagents: number
  bytes: number
  oldest: number
}

const found: Found[] = harnessOrder.map((id) => {
  const chats = archive.chats.filter((c) => c.harness === id)
  return {
    id,
    chats: chats.length,
    subagents: chats.reduce((s, c) => s + c.subagentCount, 0),
    // Subagent files and tool outputs sit beside the transcript.
    bytes: chats.reduce((s, c) => s + c.sizeBytes * (1 + c.subagentCount * 0.08), 0),
    oldest: Math.min(...chats.map((c) => c.started)),
  }
})

const steps = ['看會讀什麼', '選擇', '保存'] as const

function Steps({ at }: { at: number }) {
  return (
    <ol className="flex items-center gap-2 text-xs">
      {steps.map((label, i) => (
        <li key={label} className="flex items-center gap-2">
          <span
            className={cn(
              'flex size-5 items-center justify-center rounded-full border text-[11px] font-medium tabular',
              i < at && 'border-transparent bg-foreground text-background',
              i === at && 'border-foreground text-foreground',
              i > at && 'text-muted-foreground',
            )}
          >
            {i < at ? <Check className="size-3" /> : i + 1}
          </span>
          <span className={i === at ? 'font-medium text-foreground' : 'text-muted-foreground'}>{label}</span>
          {i < steps.length - 1 && <span className="h-px w-6 bg-border" />}
        </li>
      ))}
    </ol>
  )
}

function Preview() {
  const total = found.reduce((s, f) => s + f.bytes, 0)
  return (
    <div className="flex flex-col gap-4">
      <div className="overflow-hidden rounded-lg border">
        <table className="w-full text-[13px]">
          <thead className="bg-muted/60 text-xs text-muted-foreground">
            <tr className="[&_th]:px-3 [&_th]:py-2 [&_th]:text-left [&_th]:font-medium">
              <th>工具</th>
              <th>資料夾</th>
              <th className="text-right!">對話</th>
              <th className="text-right!">大小</th>
              <th className="text-right!">最早</th>
            </tr>
          </thead>
          <tbody>
            {found.map((f) => (
              <tr key={f.id} className="border-t [&_td]:px-3 [&_td]:py-2">
                <td className="whitespace-nowrap">
                  <HarnessBadge id={f.id} withName />
                </td>
                <td className="font-mono text-xs text-muted-foreground">{harnesses[f.id].root}</td>
                <td className="text-right whitespace-nowrap tabular">
                  {num(f.chats)}
                  {f.subagents > 0 && <span className="block text-xs text-muted-foreground">另有 {num(f.subagents)} 個 subagent</span>}
                </td>
                <td className="text-right whitespace-nowrap tabular">{bytes(f.bytes)}</td>
                <td className="text-right whitespace-nowrap tabular">{date(f.oldest, true)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="text-xs text-muted-foreground">
        到這裡為止只數了檔案的數量、大小和日期，還沒有打開任何一段對話。
      </p>
      <div className="grid grid-cols-2 gap-3 text-[13px] max-[640px]:grid-cols-1">
        <div className="flex flex-col gap-2 rounded-lg border bg-card p-3.5">
          <span className="flex items-center gap-1.5 font-medium">
            <FolderSearch className="size-4 text-muted-foreground" />
            會讀取
          </span>
          <ul className="flex list-disc flex-col gap-1 pl-5 text-muted-foreground">
            <li>對話紀錄檔，含 subagent 的紀錄</li>
            <li>工具輸出和貼上的附件</li>
            <li>專案資料夾的 .git：位置、分支、遠端網址</li>
          </ul>
        </div>
        <div className="flex flex-col gap-2 rounded-lg border bg-card p-3.5">
          <span className="flex items-center gap-1.5 font-medium">
            <ShieldCheck className="size-4 text-muted-foreground" />
            不會做
          </span>
          <ul className="flex list-disc flex-col gap-1 pl-5 text-muted-foreground">
            <li>不寫入、不搬動、不刪除工具自己的資料夾</li>
            <li>不啟動任何工具，不讀專案裡的程式碼</li>
            <li>不傳到任何地方</li>
          </ul>
        </div>
      </div>
      <dl className="grid grid-cols-3 gap-3 rounded-lg bg-muted/60 px-3.5 py-3 text-[13px] max-[640px]:grid-cols-1">
        <div>
          <dt className="text-xs text-muted-foreground">存檔預計多出</dt>
          {/* The core crate's archive is ≈2.9× the source today; the real app asks the backend for this estimate. */}
          <dd className="font-medium tabular">約 {bytes(total * 2.9)}</dd>
        </div>
        <div>
          <dt className="text-xs text-muted-foreground">這顆磁碟還有</dt>
          <dd className="font-medium tabular">186 GB</dd>
        </div>
        <div>
          <dt className="flex items-center gap-1 text-xs text-muted-foreground">
            <Lock className="size-3" />
            加密
          </dt>
          <dd className="font-medium">跟現在的存檔一樣</dd>
        </div>
      </dl>
    </div>
  )
}

function Choose({
  picked,
  setPicked,
  git,
  setGit,
  navNote,
}: {
  picked: Set<HarnessId>
  setPicked: (next: Set<HarnessId>) => void
  git: boolean
  setGit: (on: boolean) => void
  navNote: string
}) {
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-col divide-y rounded-lg border">
        {found.map((f) => (
          <label key={f.id} className="flex cursor-pointer items-center gap-3 px-3.5 py-3 text-[13px]">
            <Checkbox
              checked={picked.has(f.id)}
              onCheckedChange={(on) => {
                const next = new Set(picked)
                if (on) next.add(f.id)
                else next.delete(f.id)
                setPicked(next)
              }}
            />
            <HarnessBadge id={f.id} withName />
            <span className="ml-auto text-xs text-muted-foreground tabular">
              {num(f.chats)} 段 · {bytes(f.bytes)}
            </span>
          </label>
        ))}
      </div>
      <label className="flex items-start justify-between gap-4 rounded-lg border bg-card px-3.5 py-3 text-[13px]">
        <span className="flex flex-col gap-1">
          <span className="flex items-center gap-1.5 font-medium">
            <GitBranch className="size-4 text-muted-foreground" />
            記下專案的 git 資訊
          </span>
          <span className="leading-relaxed text-muted-foreground">
            worktree 刪掉以後就查不到它屬於哪個專案，所以要在備份當下記下來。只讀 .git 的位置、目前分支和遠端網址，不讀程式碼。
          </span>
        </span>
        <Switch checked={git} onCheckedChange={setGit} className="mt-0.5" />
      </label>
      <ul className="flex flex-col gap-1.5 text-[13px] text-muted-foreground">
        <li>排程跟瀏覽紀錄一樣，每小時一次。</li>
        <li>{navNote}</li>
        <li>之後可以在「設定 › Agent 對話」改這些選擇，或整個關掉；關掉不會刪掉已經存下的對話。</li>
      </ul>
    </div>
  )
}

function Run({ picked, progress, done }: { picked: HarnessId[]; progress: Record<string, number>; done: boolean }) {
  return (
    <div className="flex flex-col gap-4">
      {picked.map((id) => {
        const f = found.find((x) => x.id === id)!
        const p = progress[id] ?? 0
        return (
          <div key={id} className="flex flex-col gap-2">
            <div className="flex items-center justify-between text-[13px]">
              <HarnessBadge id={id} withName />
              <span className="flex items-center gap-1.5 text-xs text-muted-foreground tabular">
                {p >= 1 ? (
                  <>
                    <CircleCheck className="size-3.5 text-green" />
                    {num(f.chats)} 段都存好了
                  </>
                ) : (
                  `已保存 ${num(Math.floor(f.chats * p))} / ${num(f.chats)} 段`
                )}
              </span>
            </div>
            <Progress value={p * 100} />
          </div>
        )
      })}
      {!done && <p className="text-xs text-muted-foreground">關掉這個視窗也會繼續存，進度在左下角的背景任務裡看得到。</p>}
    </div>
  )
}

export function EnableAgentDialog() {
  const [params, setParams] = useSearchParams()
  const navigate = useNavigate()
  const { set, enabledKinds, flatten, nav } = useProto()
  const switchTo = useSwitchSpace()
  const open = params.get('enable') === 'agent'
  const [step, setStep] = useState(0)
  const [picked, setPicked] = useState<Set<HarnessId>>(() => new Set(harnessOrder))
  const [git, setGit] = useState(true)
  const [progress, setProgress] = useState<Record<string, number>>({})
  const enabledOnce = useRef(false)
  const pickedList = useMemo(() => harnessOrder.filter((h) => picked.has(h)), [picked])
  const done = step === 2 && pickedList.every((h) => (progress[h] ?? 0) >= 1)

  const others = enabledKinds.filter((k) => k !== 'agent')
  const navNote =
    nav === 'spaces'
      ? `開啟後，左上角的切換選單會多一個「Agent 對話」${others.length ? `（${spaceKeyLabel(others.length + 1)} 也能切過去）` : ''}，切過去有對話、專案、洞察和它自己的備份頁。`
      : others.length === 0 && flatten
        ? '開啟後，導航列會直接換成「對話、專案、洞察」。'
        : '開啟後，導航列會多一個「Agent」，點進去有對話、專案、洞察三頁。'

  useEffect(() => {
    if (!open) {
      setStep(0)
      setProgress({})
      enabledOnce.current = false
    }
  }, [open])

  useEffect(() => {
    if (step !== 2) return
    const speed: Record<HarnessId, number> = { claude: 0.09, codex: 0.06, pi: 0.3, deepseek: 0.5 }
    const timer = setInterval(() => {
      setProgress((prev) => {
        const next = { ...prev }
        for (const h of pickedList) next[h] = Math.min(1, (next[h] ?? 0) + speed[h] * (0.6 + Math.random() * 0.8))
        return next
      })
    }, 220)
    return () => clearInterval(timer)
  }, [step, pickedList])

  // Turn the type on once, when the first backup finishes (set changes identity on every update).
  useEffect(() => {
    if (done && !enabledOnce.current) {
      enabledOnce.current = true
      set({ agentEnabled: true })
    }
  }, [done, set])

  const close = () => {
    const next = new URLSearchParams(params)
    next.delete('enable')
    setParams(next, { replace: true })
  }

  const totalChats = found.filter((f) => picked.has(f.id)).reduce((s, f) => s + f.chats, 0)
  const totalSubs = found.filter((f) => picked.has(f.id)).reduce((s, f) => s + f.subagents, 0)
  const totalBytes = found.filter((f) => picked.has(f.id)).reduce((s, f) => s + f.bytes, 0)

  return (
    <Dialog open={open} onOpenChange={(o) => !o && close()}>
      <DialogContent className="gap-5 sm:max-w-[760px]">
        <DialogHeader className="gap-3">
          <Steps at={done ? 3 : step} />
          <DialogTitle className="text-lg">
            {step === 0 ? '保存 Agent 對話：先看看會讀什麼' : step === 1 ? '要保存哪些工具的對話？' : done ? '存好了' : '正在保存'}
          </DialogTitle>
          <DialogDescription>
            {step === 0
              ? 'Claude Code 預設 30 天後刪掉舊對話，Codex 的紀錄也常被清掉。開啟後，PathKeep 把它們存進你的存檔，照專案、時間和全文都找得回來。'
              : step === 1
                ? '沒勾的工具不會被讀取。'
                : done
                  ? `保存了 ${num(totalChats)} 段對話和 ${num(totalSubs)} 個 subagent，共 ${bytes(totalBytes)}。`
                  : '第一次要讀完全部的紀錄，之後只讀新增的部分。'}
          </DialogDescription>
        </DialogHeader>
        {step === 0 && <Preview />}
        {step === 1 && <Choose picked={picked} setPicked={setPicked} git={git} setGit={setGit} navNote={navNote} />}
        {step === 2 && <Run picked={pickedList} progress={progress} done={done} />}
        <DialogFooter>
          {step === 0 && (
            <>
              <Button variant="ghost" onClick={close}>
                不用了
              </Button>
              <Button onClick={() => setStep(1)}>下一步</Button>
            </>
          )}
          {step === 1 && (
            <>
              <Button variant="ghost" onClick={() => setStep(0)}>
                上一步
              </Button>
              <Button disabled={picked.size === 0} onClick={() => setStep(2)}>
                開始保存 {num(totalChats)} 段
              </Button>
            </>
          )}
          {step === 2 &&
            (done ? (
              <>
                <Button variant="ghost" onClick={close}>
                  關閉
                </Button>
                <Button
                  onClick={() => {
                    close()
                    set({ agentEnabled: true })
                    // Switch to the Agent space so the rail and tint change with the page.
                    if (nav === 'spaces') switchTo('agent', '/agent/chats')
                    else void navigate('/agent/chats')
                  }}
                >
                  看對話
                </Button>
              </>
            ) : (
              <Button
                variant="outline"
                onClick={() => {
                  set({ agentEnabled: true })
                  toast('在背景繼續保存', { description: '進度在左下角的背景任務裡看得到。' })
                  close()
                }}
              >
                <X />
                在背景繼續
              </Button>
            ))}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
