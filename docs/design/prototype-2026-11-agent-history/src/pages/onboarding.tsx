/**
 * First run, full window, as src/features/onboarding/onboarding-page.tsx:
 * step list on the left with language and light/dark, a thin progress bar on
 * top, one step at a time on the right. Same steps as today's onboarding,
 * except that today's "瀏覽器" step becomes two: "要保存什麼" (which types) and
 * "來源" (each chosen type's sources), per data-spaces.md §3.3.
 */
import { ArrowLeft, ArrowRight, Check } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import darkMark from '@/assets/pathkeep-mark-dark.svg'
import lightMark from '@/assets/pathkeep-mark.svg'
import { Button } from '@/components/ui/button'
import { Spinner } from '@/components/ui/spinner'
import { Switch } from '@/components/ui/switch'
import { cn } from '@/lib/cn'
import { kindById, type KindId } from '~/kinds'
import { useProto } from '~/proto-state'
import { aiTitles, AiStep } from './onboarding/ai-step'
import { AppearanceControls } from './onboarding/appearance'
import { DoneStep, summaryIcons, type SummaryRow } from './onboarding/done-step'
import { backgroundCanUnlock, draftStepReady, initialDraft, type Draft } from './onboarding/draft'
import { EncryptionStep } from './onboarding/encryption-step'
import { defaultCases, OnboardingSwitches, type OnboardingCases } from './onboarding/proto-switches'
import { frequencyTitles, ScheduleStep } from './onboarding/schedule-step'
import { initialSelection, stepKinds } from './onboarding/sources-data'
import { type Scan, SourcesStep, scanned, sourcesComplete } from './onboarding/sources-step'
import { KindsStep, StepPosition, WelcomeStep } from './onboarding/steps'
import { APP_ROOT, StorageStep } from './onboarding/storage-step'
import { stagesFor, useFinishSetup } from './onboarding/use-finish'

const steps = [
  { id: 'welcome', label: '歡迎' },
  { id: 'kinds', label: '要保存什麼' },
  { id: 'sources', label: '來源' },
  { id: 'storage', label: '存放位置' },
  { id: 'encryption', label: '加密' },
  { id: 'schedule', label: '自動備份' },
  { id: 'ai', label: 'AI' },
  { id: 'done', label: '完成' },
] as const

const twoKinds: KindId[] = ['browsing', 'agent']
const allKinds: KindId[] = ['browsing', 'agent', 'terminal', 'messages', 'webchat', 'notifications']

export default function OnboardingPage() {
  const navigate = useNavigate()
  const { theme, set } = useProto()
  const [params] = useSearchParams()
  // `?step=sources` opens a step directly, also when only the address changes.
  const requested = steps.findIndex((s) => s.id === params.get('step'))
  const [index, setIndex] = useState(Math.max(0, requested))
  useEffect(() => {
    if (requested >= 0) setIndex(requested)
  }, [requested])
  // Prototype-only: many types and sources at once, to judge how the steps hold up.
  const [many, setMany] = useState(params.get('many') === '1')
  const [kindsOn, setKindsOn] = useState<Set<KindId>>(new Set(many ? allKinds : twoKinds))
  const [cases, setCases] = useState<OnboardingCases>(defaultCases)
  // What looking at this computer found for each chosen type.
  const found = useMemo(
    () => stepKinds(kindsOn, many).map((kind) => (kind.id === 'agent' && !cases.agentTools ? { ...kind, sources: [] } : kind)),
    [kindsOn, many, cases.agentTools],
  )
  const notFound = useMemo(() => new Set<KindId>(cases.agentTools ? [] : ['agent']), [cases.agentTools])
  // Prototype-only: what looking for browsers came back with, and whether Safari's access was granted since.
  const [scan, setScan] = useState<Scan>('found')
  const [granted, setGranted] = useState(false)
  const list = useMemo(() => scanned(found, scan, granted), [found, scan, granted])
  const [picked, setPicked] = useState<Record<string, Set<string>>>({})
  // Every source that was readable when the step first looked starts out selected; a choice already made is kept.
  const selection = useMemo(() => ({ ...initialSelection(found), ...picked }), [found, picked])
  const [draft, setDraft] = useState<Draft>(() => initialDraft(defaultCases.keychainAvailable))
  const update = (patch: Partial<Draft>) => setDraft((current) => ({ ...current, ...patch }))
  const where = cases.mac ? '在這台 Mac 上' : '在這台電腦上'

  const stages = useMemo(() => stagesFor(draft, { keychainAvailable: cases.keychainAvailable, scheduleSupported: cases.preview !== 'unsupported' }), [draft, cases.keychainAvailable, cases.preview])
  const finish = useFinishSetup(stages, cases.failAt, list, selection)
  const locked = finish.state.status !== 'idle'

  const toggle = <T,>(setter: React.Dispatch<React.SetStateAction<Set<T>>>, value: T) =>
    setter((s) => {
      const n = new Set(s)
      if (n.has(value)) n.delete(value)
      else n.add(value)
      return n
    })

  const step = steps[index].id
  const ready = step === 'kinds' ? kindsOn.size > 0 : step === 'sources' ? sourcesComplete(list, selection) : draftStepReady(step, draft)

  const enter = () => {
    const both = kindsOn.has('browsing') && kindsOn.has('agent')
    set({ preset: both ? 'two' : kindsOn.has('agent') ? 'agent' : 'browsing', agentEnabled: true })
    void navigate('/')
  }
  const next = () => {
    if (!ready) return
    if (step !== 'done') return setIndex(index + 1)
    if (finish.state.status === 'done') return enter()
    if (finish.state.status === 'idle') finish.start()
  }
  const skip = () => {
    update(step === 'schedule' ? { frequency: 'off' } : { ai: 'off' })
    setIndex(index + 1)
  }

  const keychainStage = stages.includes('keychain')
  const summary: SummaryRow[] = [
    { icon: summaryIcons.kinds, label: '保存', value: [...kindsOn].map((id) => kindById[id].label).join('、') },
    ...list.map((kind) => ({
      icon: kind.id === 'browsing' ? summaryIcons.browsing : kind.id === 'agent' ? summaryIcons.agent : kind.icon,
      label: kind.id === 'browsing' ? '瀏覽器' : kind.id === 'agent' ? 'Agent 工具' : kind.label,
      value: `${kind.sources.filter((source) => selection[kind.id]?.has(source.id) && !source.blocked).length} 個${kind.alsoReads ? `，${kind.alsoReads}` : ''}`,
    })),
    { icon: summaryIcons.location, label: '存放位置', value: APP_ROOT },
    { icon: summaryIcons.encryption, label: '加密', value: draft.encrypt ? ['已開啟 · AES-256', keychainStage && '密碼在鑰匙圈中'].filter(Boolean).join(' · ') : '不加密' },
    { icon: summaryIcons.schedule, label: '自動備份', value: frequencyTitles[draft.frequency] },
    { icon: summaryIcons.ai, label: 'AI', value: aiTitles[draft.ai] },
    { icon: summaryIcons.online, label: '連網', value: '會下載每個頁面的預覽圖 · 可在「設定 → 瀏覽紀錄」關閉' },
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
              return (
                <li key={s.id}>
                  <button
                    type="button"
                    disabled={!done || locked}
                    aria-current={current ? 'step' : undefined}
                    onClick={() => setIndex(position)}
                    className={cn(
                      'flex h-10 w-full items-center gap-3 rounded-[10px] px-2.5 text-left text-sm font-medium transition-[background-color,color,box-shadow] duration-200 disabled:cursor-default',
                      current ? 'bg-card text-foreground shadow-card dark:bg-popover' : done ? 'text-foreground hover:bg-muted' : 'text-muted-foreground',
                    )}
                  >
                    <span
                      className={cn(
                        'flex size-[22px] shrink-0 items-center justify-center rounded-full border-[1.5px] font-mono text-[11px] font-semibold transition-colors duration-200',
                        done ? 'border-primary bg-primary text-primary-foreground' : current ? 'border-foreground' : 'border-border',
                      )}
                    >
                      {done ? <Check className="size-3" strokeWidth={3} aria-hidden /> : position + 1}
                    </span>
                    {s.label}
                    {done && <span className="sr-only"> (已完成)</span>}
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
        <OnboardingSwitches
          cases={cases}
          onChange={(patch) => {
            setCases((c) => ({ ...c, ...patch }))
            // A type with nothing found starts out unticked; one that is found starts out ticked.
            if (patch.agentTools !== undefined) setKindsOn((on) => new Set(patch.agentTools ? [...on, 'agent'] : [...on].filter((id) => id !== 'agent')))
          }}
          locked={locked}
        />
        <button type="button" className="mb-3 rounded-lg border border-dashed px-2.5 py-2 text-left text-xs text-muted-foreground hover:bg-muted" onClick={() => void navigate('/')}>
          原型：離開新手引導
        </button>
        <AppearanceControls />
      </aside>
      <main className="my-2.5 mr-2.5 flex min-w-0 flex-1 flex-col overflow-hidden rounded-xl border bg-panel">
        <div className="h-[3px] shrink-0 bg-muted" role="progressbar" aria-label="設定進度" aria-valuemin={1} aria-valuemax={steps.length} aria-valuenow={index + 1}>
          <div className="h-full bg-brand transition-[width] duration-300 ease-out" style={{ width: `${((index + 1) / steps.length) * 100}%` }} />
        </div>
        <StepPosition.Provider value={{ index, total: steps.length }}>
          <StepScroller index={index} wide={step === 'sources'}>
            {step === 'welcome' && <WelcomeStep />}
            {step === 'kinds' && <KindsStep selected={kindsOn} onToggle={(id) => toggle(setKindsOn, id)} pickAll={many} notFound={notFound} />}
            {step === 'sources' && <SourcesStep
                key={many ? 'many' : 'few'}
                list={list}
                selection={selection}
                onSelection={(id, next) => setPicked((all) => ({ ...all, [id]: next }))}
                scan={scan}
                onScan={setScan}
                onGranted={() => setGranted(true)}
                onFound={(id) => id === 'agent' && setCases((c) => ({ ...c, agentTools: true }))}
              />}
            {step === 'storage' && <StorageStep list={list} selection={selection} mac={cases.mac} />}
            {step === 'encryption' && <EncryptionStep draft={draft} onChange={update} keychainAvailable={cases.keychainAvailable} mac={cases.mac} />}
            {step === 'schedule' && (
              <ScheduleStep
                frequency={draft.frequency}
                onChange={(frequency) => update({ frequency })}
                backgroundCanUnlock={backgroundCanUnlock(draft, cases.keychainAvailable)}
                preview={cases.preview}
                mac={cases.mac}
              />
            )}
            {step === 'ai' && <AiStep draft={draft} where={where} onChange={update} listCase={cases.listCase} />}
            {step === 'done' && (
              <DoneStep summary={summary} stages={stages} finish={finish.state} onRetry={finish.start} onSkip={finish.skip} localAi={draft.ai === 'local'} result={cases.result} />
            )}

            <footer className="flex items-center justify-between pt-2">
              <Button variant="ghost" className={cn('h-[38px] px-3.5', index === 0 && 'invisible')} disabled={index === 0 || locked} onClick={() => setIndex(index - 1)}>
                <ArrowLeft />
                上一步
              </Button>
              <div className="flex gap-2">
                {(step === 'schedule' || step === 'ai') && (
                  <Button variant="outline" className="h-[38px] px-3.5" onClick={skip}>
                    跳過
                  </Button>
                )}
                {finish.state.status !== 'failed' && (
                  <Button className="h-[38px] px-[18px]" disabled={!ready || finish.state.status === 'running'} onClick={next}>
                    {step !== 'done' ? (
                      <>
                        {index === 0 ? '開始設定' : '繼續'}
                        <ArrowRight />
                      </>
                    ) : finish.state.status === 'running' ? (
                      <>
                        <Spinner aria-hidden role={undefined} />
                        正在設定…
                      </>
                    ) : (
                      <>
                        {finish.state.status === 'done' ? '進入 PathKeep' : '開始第一次備份'}
                        <ArrowRight />
                      </>
                    )}
                  </Button>
                )}
              </div>
            </footer>
          </StepScroller>
        </StepPosition.Provider>
      </main>
    </div>
  )
}

/**
 * The scrolling content column. Each step mounts fresh so it rises in; the
 * scroll position resets and focus moves to the new heading.
 */
function StepScroller({ index, wide, children }: { index: number; wide: boolean; children: React.ReactNode }) {
  const ref = useRef<HTMLDivElement>(null)
  const first = useRef(true)
  useEffect(() => {
    const scroller = ref.current
    if (!scroller) return
    scroller.scrollTop = 0
    if (first.current) {
      first.current = false
      return
    }
    scroller.querySelector<HTMLElement>('[data-step-heading]')?.focus({ preventScroll: true })
  }, [index])
  return (
    <div ref={ref} className="flex flex-1 overflow-y-auto">
      <div key={index} className={cn('m-auto flex w-full animate-rise flex-col gap-6 px-10 py-12 motion-reduce:animate-none', wide ? 'max-w-[960px]' : 'max-w-[620px]')}>
        {children}
      </div>
    </div>
  )
}
