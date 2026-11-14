/**
 * First run, full window: step list on the left, one step at a time on the
 * right. Same steps as today's onboarding, except that today's "瀏覽器" step
 * becomes two: "要保存什麼" (which types) and "來源" (each chosen type's sources).
 */
import { ArrowLeft, ArrowRight, Check } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { toast } from 'sonner'
import darkMark from '@/assets/pathkeep-mark-dark.svg'
import lightMark from '@/assets/pathkeep-mark.svg'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/cn'
import { kindById, type KindId } from '~/kinds'
import { useProto } from '~/proto-state'
import { Switch } from '@/components/ui/switch'
import { initialSelection, stepKinds } from './onboarding/sources-data'
import { SourcesStep, sourcesComplete } from './onboarding/sources-step'
import {
  AiStep,
  DoneStep,
  EncryptionStep,
  KindsStep,
  ScheduleStep,
  StorageStep,
  WelcomeStep,
  type AiChoice,
  type Encryption,
  type Schedule,
} from './onboarding/steps'

const steps = [
  { id: 'welcome', label: '歡迎' },
  { id: 'kinds', label: '要保存什麼' },
  { id: 'sources', label: '來源' },
  { id: 'storage', label: '存在哪裡' },
  { id: 'encryption', label: '加密' },
  { id: 'schedule', label: '排程' },
  { id: 'ai', label: 'AI' },
  { id: 'done', label: '完成' },
] as const

const twoKinds: KindId[] = ['browsing', 'agent']
const allKinds: KindId[] = ['browsing', 'agent', 'terminal', 'messages', 'webchat', 'notifications']

const scheduleLabel: Record<Schedule, string> = { '1h': '每小時', '6h': '每 6 小時', '1d': '每天', manual: '手動' }

export default function OnboardingPage() {
  const navigate = useNavigate()
  const { theme, set } = useProto()
  const [params] = useSearchParams()
  // `?step=sources` opens a step directly, also when only the address changes.
  const requested = steps.findIndex((s) => s.id === params.get('step'))
  const initial = Math.max(0, requested)
  const [index, setIndex] = useState(initial)
  const [reached, setReached] = useState(initial)
  useEffect(() => {
    if (requested < 0) return
    setIndex(requested)
    setReached((r) => Math.max(r, requested))
  }, [requested])
  // Prototype-only: many types and sources at once, to judge how the steps hold up.
  const [many, setMany] = useState(params.get('many') === '1')
  const [kindsOn, setKindsOn] = useState<Set<KindId>>(new Set(many ? allKinds : twoKinds))
  const list = useMemo(() => stepKinds(kindsOn, many), [kindsOn, many])
  const [picked, setPicked] = useState<Record<string, Set<string>>>({})
  // A type's sources start out all selected; a choice already made is kept.
  const selection = useMemo(() => ({ ...initialSelection(list), ...picked }), [list, picked])
  const [encryption, setEncryption] = useState<Encryption>('password')
  const [schedule, setSchedule] = useState<Schedule>('1h')
  const [ai, setAi] = useState<AiChoice>('none')
  const scroller = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (scroller.current) scroller.current.scrollTop = 0
  }, [index])

  const toggle = <T,>(setter: React.Dispatch<React.SetStateAction<Set<T>>>, value: T) =>
    setter((s) => {
      const n = new Set(s)
      if (n.has(value)) n.delete(value)
      else n.add(value)
      return n
    })

  const go = (next: number) => {
    setIndex(next)
    setReached((r) => Math.max(r, next))
  }
  const step = steps[index].id
  const canNext = step === 'kinds' ? kindsOn.size > 0 : step !== 'sources' || sourcesComplete(list, selection)

  const finish = () => {
    const both = kindsOn.has('browsing') && kindsOn.has('agent')
    set({ preset: both ? 'two' : kindsOn.has('agent') ? 'agent' : 'browsing', agentEnabled: true })
    toast('原型：開始第一次備份')
    void navigate('/')
  }

  const summary = [
    { label: '保存', value: [...kindsOn].map((id) => kindById[id].label).join('、') },
    ...list.map((kind) => ({
      label: kind.id === 'browsing' ? '瀏覽器設定檔' : kind.id === 'agent' ? 'Agent 工具' : kind.label,
      value: `${selection[kind.id]?.size ?? 0} 個${kind.alsoReads ? `，${kind.alsoReads}` : ''}`,
    })),
    { label: '存檔', value: <span className="font-mono text-[12px]">~/Library/Application Support/PathKeep</span> },
    { label: '加密', value: encryption === 'password' ? '用密碼加密' : '不加密' },
    { label: '排程', value: scheduleLabel[schedule] },
  ]

  return (
    <div className="relative flex h-full bg-window backdrop-blur-[40px] backdrop-saturate-[1.4]">
      <div aria-hidden className="absolute top-[17px] left-[18px] flex gap-2">
        <span className="size-3 rounded-full bg-[#ff5f57]" />
        <span className="size-3 rounded-full bg-[#febc2e]" />
        <span className="size-3 rounded-full bg-[#28c840]" />
      </div>
      <aside className="flex w-[260px] shrink-0 flex-col gap-1 px-5 pb-5">
        <div className="h-[52px] shrink-0" />
        <div className="flex items-center gap-2.5 px-2 pb-6">
          <img src={theme === 'dark' ? darkMark : lightMark} alt="" className="size-7" />
          <span className="text-base font-semibold tracking-[-0.01em]">PathKeep</span>
        </div>
        <nav aria-label="設定步驟">
          <ol className="flex flex-col gap-1">
            {steps.map((s, position) => {
              const done = position < index
              const current = position === index
              const reachable = position <= reached
              return (
                <li key={s.id}>
                  <button
                    type="button"
                    disabled={!reachable || current}
                    aria-current={current ? 'step' : undefined}
                    onClick={() => setIndex(position)}
                    className={cn(
                      'flex h-10 w-full items-center gap-3 rounded-[10px] px-2.5 text-left text-sm font-medium transition-[background-color,color,box-shadow] duration-200 disabled:cursor-default',
                      current ? 'bg-card text-foreground shadow-card dark:bg-popover' : reachable ? 'text-foreground hover:bg-muted' : 'text-muted-foreground',
                    )}
                  >
                    <span
                      className={cn(
                        'flex size-[22px] shrink-0 items-center justify-center rounded-full border-[1.5px] font-mono text-[11px] font-semibold',
                        done ? 'border-primary bg-primary text-primary-foreground' : current ? 'border-foreground' : 'border-border',
                      )}
                    >
                      {done ? <Check className="size-3" strokeWidth={3} /> : position + 1}
                    </span>
                    {s.label}
                  </button>
                </li>
              )
            })}
          </ol>
        </nav>
        <div className="flex-1" />
        <label className="mb-1 flex items-center justify-between gap-3 rounded-lg border border-dashed px-2.5 py-2 text-xs text-muted-foreground">
          原型：很多類型和來源
          <Switch
            checked={many}
            onCheckedChange={(on) => {
              setMany(on)
              setPicked({})
              setKindsOn(new Set(on ? allKinds : twoKinds))
            }}
            aria-label="原型：很多類型和來源"
          />
        </label>
        <Button variant="ghost" size="sm" className="w-fit text-muted-foreground" onClick={() => void navigate('/')}>
          離開原型的新手引導
        </Button>
      </aside>
      <main className="mt-2.5 mr-2.5 mb-2.5 flex min-w-0 flex-1 flex-col overflow-hidden rounded-xl border bg-panel">
        <div ref={scroller} className="flex flex-1 overflow-y-auto">
          <div key={index} className={cn('m-auto flex w-full animate-rise flex-col gap-6 px-10 py-12', step === 'sources' ? 'max-w-[960px]' : 'max-w-[620px]')}>
            {step === 'welcome' && <WelcomeStep />}
            {step === 'kinds' && <KindsStep selected={kindsOn} onToggle={(id) => toggle(setKindsOn, id)} pickAll={many} />}
            {step === 'sources' && <SourcesStep key={many ? 'many' : 'few'} list={list} selection={selection} onSelection={(id, next) => setPicked((all) => ({ ...all, [id]: next }))} />}
            {step === 'storage' && <StorageStep kindsOn={kindsOn} />}
            {step === 'encryption' && <EncryptionStep value={encryption} onChange={setEncryption} />}
            {step === 'schedule' && <ScheduleStep value={schedule} onChange={setSchedule} />}
            {step === 'ai' && <AiStep value={ai} onChange={setAi} />}
            {step === 'done' && <DoneStep summary={summary} />}
          </div>
        </div>
        <footer className="flex shrink-0 items-center justify-between border-t px-10 py-4">
          <Button variant="ghost" onClick={() => go(index - 1)} disabled={index === 0}>
            <ArrowLeft />
            上一步
          </Button>
          {step === 'done' ? (
            <Button onClick={finish}>
              開始第一次備份
              <ArrowRight />
            </Button>
          ) : (
            <Button onClick={() => go(index + 1)} disabled={!canNext}>
              {step === 'welcome' ? '開始設定' : '下一步'}
              <ArrowRight />
            </Button>
          )}
        </footer>
      </main>
    </div>
  )
}
