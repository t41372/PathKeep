/**
 * First-run setup: eight steps from welcome to the first backup, then hand
 * off to the main app.
 *
 * Responsible for: the window layout (step list, progress bar, language and
 * theme), moving between steps, holding the draft (which kinds of data,
 * which of their sources, and the rest), the walk of the Agent tool folders
 * the later steps share, and saving the choices so an interrupted setup
 * resumes with them: the kinds and Agent tools (`save_onboarding_choices`,
 * on Continue from the kinds and sources steps) and the browsers (the
 * config). Continue waits while they save; a failed save keeps the step.
 * Not responsible for: what each step shows (the `*-step.tsx` files) or
 * creating the archive (see `use-finish-setup.ts`).
 */
import { useQueryClient } from '@tanstack/react-query'
import { ArrowLeft, ArrowRight, Check, Moon, Sun } from 'lucide-react'
import { useEffect, useEffectEvent, useMemo, useRef, useState } from 'react'
import { toast } from 'sonner'
import { useSession } from '@/app/session'
import { panelTopMargin } from '@/app/window-chrome/frame'
import { BrandMark } from '@/components/app/brand-mark'
import { backdropDragRegion } from '@/components/app/drag-region'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Spinner } from '@/components/ui/spinner'
import { inToolOrder } from '@/features/data-kinds/tools'
import {
  dataKindsClient,
  type DataKindId,
  type OnboardingChoices,
} from '@/lib/backend-client/data-kinds'
import { cn } from '@/lib/cn'
import { describeError } from '@/lib/errors'
import { languageNames, supportedLanguages, useI18n } from '@/lib/i18n'
import { queryKeys } from '@/lib/query'
import { currentSnapshot, useSaveConfig, useSnapshot } from '@/lib/queries/app'
import { useDetectedSources } from '@/lib/queries/data-kinds'
import { hasMacOverlayTitlebar, isMacOsHost } from '@/lib/runtime'
import { spaceBackupPath } from '@/lib/spaces'
import { useTheme } from '@/lib/theme'
import type { LanguagePreference } from '@/lib/types'
import { AiStep } from './ai-step'
import { DoneStep } from './done-step'
import {
  backgroundCanUnlock,
  canContinue,
  choicesToSave,
  chosenKinds,
  initialDraft,
  kindsLackingSource,
  steps,
  type Draft,
  type StepContext,
} from './draft'
import { EncryptionStep } from './encryption-step'
import { InlineError } from './inline-error'
import { KindsStep } from './kinds-step'
import { ScheduleStep } from './schedule-step'
import { SourcesStep } from './sources-step'
import { useSchedulePreview } from './use-schedule-preview'
import { StorageStep } from './storage-step'
import {
  chosenReadableTools,
  chosenTools,
  useAgentSources,
} from './use-agent-sources'
import { stagesFor, useFinishSetup } from './use-finish-setup'
import { WelcomeStep } from './welcome-step'

const AGENT_KINDS: DataKindId[] = ['agent-history']

const pad = (value: number) => String(value).padStart(2, '0')

export default function OnboardingPage() {
  const { t } = useI18n()
  const session = useSession()
  const client = useQueryClient()
  const snapshot = useSnapshot()
  const save = useSaveConfig()
  const [index, setIndex] = useState(0)
  const [draft, setDraft] = useState<Draft>(() => initialDraft(snapshot))
  const [saveError, setSaveError] = useState<string | null>(null)
  const [savingChoices, setSavingChoices] = useState(false)
  const step = steps[index]
  const where = t(
    isMacOsHost() ? 'onboarding.where.mac' : 'onboarding.where.other',
  )

  // Only whether the tool folders are there; nothing inside is opened.
  const detection = useDetectedSources(AGENT_KINDS)
  const kinds = chosenKinds(draft, snapshot, {
    settled: !detection.isPending,
    detected: detection.data,
  })
  const agentChosen = kinds?.includes('agent-history') ?? false
  // Walked once the sources step is reached; the later steps read the same walk.
  const agent = useAgentSources(
    agentChosen && index >= steps.indexOf('sources'),
  )
  const agentTools = chosenTools(draft, agent)
  const agentReadable = chosenReadableTools(draft, agent)
  const context: StepContext = {
    snapshot,
    kinds,
    agentReadable: agentChosen ? agentReadable : [],
  }

  const preview = useSchedulePreview(
    draft.frequency,
    index >= steps.indexOf('schedule'),
  )
  const stages = useMemo(
    () =>
      stagesFor(draft, {
        keychainAvailable: snapshot.keyringStatus.available,
        scheduleSupported: preview.data?.applySupported ?? true,
      }),
    [draft, preview.data?.applySupported, snapshot.keyringStatus.available],
  )
  const finish = useFinishSetup(draft, stages, {
    kinds: kinds ?? [],
    agentTools: agentReadable ?? [],
  })
  const locked = finish.state.status !== 'idle'
  const ready = canContinue(step, draft, context)

  const update = (patch: Partial<Draft>) =>
    setDraft((current) => ({ ...current, ...patch }))

  function go(next: number) {
    setSaveError(null)
    setIndex(next)
  }

  function toggleKind(kind: DataKindId) {
    if (!kinds) return
    update({
      kinds: kinds.includes(kind)
        ? kinds.filter((item) => item !== kind)
        : [...kinds, kind].sort((a, b) =>
            a === b ? 0 : a === 'browsing' ? -1 : 1,
          ),
    })
  }

  /** Opens the app, with the kinds' states as they are now, so it lands in a space that is on. */
  async function enterApp() {
    await client
      .fetchQuery({
        queryKey: queryKeys.dataKinds,
        queryFn: dataKindsClient.list,
        staleTime: 0,
      })
      .catch(() => undefined)
    session.enter(currentSnapshot(client))
  }

  // "Keep going in the background": once the other stages are done, the
  // app opens by itself, as the enable dialog closes; the toast says where
  // the save goes on.
  const [leaving, setLeaving] = useState(false)
  function keepGoing() {
    setLeaving(true)
    finish.keepGoingInBackground()
  }
  const finished = finish.state.status === 'done'
  const leave = useEffectEvent(() => {
    toast(t('enableAgent.run.continued'), {
      description: t('enableAgent.run.continuedBody'),
    })
    void enterApp()
  })
  useEffect(() => {
    if (leaving && finished) leave()
  }, [leaving, finished])

  function openBackup(kind: DataKindId) {
    window.location.hash = spaceBackupPath(
      kind === 'browsing' ? 'browsing' : 'agent',
    )
    void enterApp()
  }

  /** Keeps the choices for a setup that stops here; false (and says why) when it can't. */
  async function keepChoices(choices: OnboardingChoices) {
    setSavingChoices(true)
    try {
      await dataKindsClient.saveOnboardingChoices(choices)
      return true
    } catch (error) {
      setSaveError(describeError(error, 'save_onboarding_choices'))
      return false
    } finally {
      setSavingChoices(false)
    }
  }

  async function next() {
    if (!ready || savingChoices) return
    // What was shown is the choice from here on, whatever a later check finds.
    if (step === 'kinds' && kinds) {
      update({ kinds })
      // No tools are chosen before the sources step: those saved earlier stay.
      if (!(await keepChoices(choicesToSave(kinds, draft.agentTools ?? []))))
        return
    }
    if (step === 'sources') {
      const tools = agentChosen
        ? inToolOrder(agentTools)
        : (draft.agentTools ?? [])
      if (agentChosen) update({ agentTools: tools })
      if (kinds && !(await keepChoices(choicesToSave(kinds, tools)))) return
      if (kinds?.includes('browsing')) {
        try {
          await save.mutateAsync((config) => ({
            ...config,
            selectedProfileIds: draft.selectedProfileIds,
          }))
        } catch (error) {
          setSaveError(describeError(error, 'save_config'))
          return
        }
      }
    }
    if (step !== 'done') return go(index + 1)
    if (finish.state.status === 'done') return enterApp()
    if (finish.state.status === 'idle') finish.start()
  }

  function skip() {
    update(step === 'schedule' ? { frequency: 'off' } : { ai: 'off' })
    go(index + 1)
  }

  return (
    // As in the archive's frame: the margin around the panel is window
    // background and drags the window; on Windows and Linux the top of it is
    // the strip the window buttons sit in.
    <div
      {...backdropDragRegion()}
      className="flex h-full bg-window backdrop-blur-[40px] backdrop-saturate-[1.4]"
    >
      <StepList
        index={index}
        locked={locked}
        onSelect={go}
        overlayTitlebar={hasMacOverlayTitlebar()}
      />
      <main
        className={cn(
          'mr-2.5 mb-2.5 flex min-w-0 flex-1 flex-col overflow-hidden rounded-xl border bg-panel',
          panelTopMargin(),
        )}
      >
        <div
          className="h-[3px] shrink-0 bg-muted"
          role="progressbar"
          aria-label={t('onboarding.progressLabel')}
          aria-valuemin={1}
          aria-valuemax={steps.length}
          aria-valuenow={index + 1}
        >
          <div
            className="h-full bg-brand transition-[width] duration-300 ease-out"
            style={{ width: `${((index + 1) / steps.length) * 100}%` }}
          />
        </div>
        <StepScroller
          index={index}
          wide={step === 'sources' && (kinds?.length ?? 0) > 1}
        >
          <header className="flex flex-col gap-2.5">
            <span className="font-mono text-xs font-medium text-brand">
              {pad(index + 1)} / {pad(steps.length)}
            </span>
            <h1
              tabIndex={-1}
              data-step-heading
              className="text-[32px] leading-[1.15] font-semibold tracking-[-0.025em] text-pretty outline-none"
            >
              {t(`onboarding.steps.${step}.title`)}
            </h1>
            <p className="text-[15px] leading-relaxed text-pretty text-muted-foreground">
              {t(`onboarding.steps.${step}.body`, { where })}
              {(step === 'encryption' || step === 'schedule') && (
                <>
                  <br />
                  {t(`onboarding.steps.${step}.note`)}
                </>
              )}
            </p>
          </header>

          {step === 'welcome' && <WelcomeStep />}
          {step === 'kinds' && (
            <KindsStep
              chosen={kinds}
              onToggle={toggleKind}
              detection={{
                pending: detection.isPending,
                detected: detection.data,
                error: detection.error,
                retry: () => void detection.refetch(),
              }}
            />
          )}
          {step === 'sources' && kinds && (
            <SourcesStep
              kinds={kinds}
              selectedProfileIds={draft.selectedProfileIds}
              onProfiles={(selectedProfileIds) =>
                update({ selectedProfileIds })
              }
              agent={agent}
              agentTools={agentTools}
              onAgentTools={(tools) => update({ agentTools: tools })}
              lacking={kindsLackingSource(draft, context)}
            />
          )}
          {step === 'storage' && kinds && (
            <StorageStep
              kinds={kinds}
              selected={draft.selectedProfileIds}
              agent={agent}
              agentTools={agentReadable ?? []}
            />
          )}
          {step === 'encryption' && (
            <EncryptionStep draft={draft} onChange={update} />
          )}
          {step === 'schedule' && (
            <ScheduleStep
              frequency={draft.frequency}
              onChange={(frequency) => update({ frequency })}
              backgroundCanUnlock={backgroundCanUnlock(draft, snapshot)}
            />
          )}
          {step === 'ai' && (
            <AiStep draft={draft} where={where} onChange={update} />
          )}
          {step === 'done' && kinds && (
            <DoneStep
              draft={draft}
              kinds={kinds}
              agentTools={agentReadable ?? []}
              stages={stages}
              finish={finish.state}
              onRetry={finish.start}
              onSkip={() => void finish.skip()}
              onOpenBackup={openBackup}
              onKeepGoing={keepGoing}
            />
          )}

          {saveError && (
            <InlineError
              title={t('onboarding.saveFailed')}
              detail={saveError}
            />
          )}

          <footer className="flex items-center justify-between pt-2">
            <Button
              variant="ghost"
              className={cn('h-[38px] px-3.5', index === 0 && 'invisible')}
              disabled={index === 0 || locked}
              onClick={() => go(index - 1)}
            >
              <ArrowLeft />
              {t('onboarding.nav.back')}
            </Button>
            <div className="flex gap-2">
              {(step === 'schedule' || step === 'ai') && (
                <Button
                  variant="outline"
                  className="h-[38px] px-3.5"
                  onClick={skip}
                >
                  {t('onboarding.nav.skip')}
                </Button>
              )}
              {finish.state.status !== 'failed' && (
                <Button
                  className="h-[38px] px-[18px]"
                  disabled={
                    !ready ||
                    save.isPending ||
                    savingChoices ||
                    finish.state.status === 'running'
                  }
                  onClick={() => void next()}
                >
                  <NextLabel
                    step={step}
                    status={finish.state.status}
                    index={index}
                  />
                </Button>
              )}
            </div>
          </footer>
        </StepScroller>
      </main>
    </div>
  )
}

function NextLabel({
  step,
  status,
  index,
}: {
  step: string
  status: string
  index: number
}) {
  const { t } = useI18n()
  if (step !== 'done')
    return (
      <>
        {t(index === 0 ? 'onboarding.nav.start' : 'onboarding.nav.next')}
        <ArrowRight />
      </>
    )
  if (status === 'running')
    return (
      <>
        <Spinner aria-hidden role={undefined} />
        {t('onboarding.done.working')}
      </>
    )
  return (
    <>
      {t(status === 'done' ? 'onboarding.done.open' : 'onboarding.done.start')}
      <ArrowRight />
    </>
  )
}

/**
 * The scrolling content column. Each step mounts fresh so it rises in; the
 * scroll position resets and focus moves to the new heading for screen
 * readers.
 */
function StepScroller({
  index,
  wide,
  children,
}: {
  index: number
  /** Room for two panes side by side (the sources step with several kinds). */
  wide: boolean
  children: React.ReactNode
}) {
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
    scroller
      .querySelector<HTMLElement>('[data-step-heading]')
      ?.focus({ preventScroll: true })
  }, [index])

  return (
    <div ref={ref} className="flex flex-1 overflow-y-auto">
      <div
        key={index}
        className={cn(
          'm-auto flex w-full animate-rise flex-col gap-6 px-10 py-12 motion-reduce:animate-none',
          wide ? 'max-w-[960px]' : 'max-w-[620px]',
        )}
      >
        {children}
      </div>
    </div>
  )
}

function StepList({
  index,
  locked,
  onSelect,
  overlayTitlebar,
}: {
  index: number
  locked: boolean
  onSelect: (index: number) => void
  overlayTitlebar: boolean
}) {
  const { t } = useI18n()
  return (
    <aside className="flex w-[260px] shrink-0 flex-col gap-1 px-5 pb-5">
      {/* Room for the traffic lights on macOS; on every desktop it drags the window. */}
      <div
        {...backdropDragRegion()}
        className={cn('shrink-0', overlayTitlebar ? 'h-[52px]' : 'h-7')}
      />
      <div className="flex items-center gap-2.5 px-2 pb-6">
        <BrandMark className="size-7" />
        <span className="text-base font-semibold tracking-[-0.01em]">
          PathKeep
        </span>
      </div>
      <nav aria-label={t('onboarding.stepsLabel')}>
        <ol className="flex flex-col gap-1">
          {steps.map((id, position) => {
            const done = position < index
            const current = position === index
            return (
              <li key={id}>
                <button
                  type="button"
                  disabled={!done || locked}
                  aria-current={current ? 'step' : undefined}
                  onClick={() => onSelect(position)}
                  className={cn(
                    'flex h-10 w-full items-center gap-3 rounded-[10px] px-2.5 text-left text-sm font-medium transition-[background-color,color,box-shadow] duration-200',
                    current
                      ? 'bg-card text-foreground shadow-card dark:bg-popover'
                      : done
                        ? 'text-foreground hover:bg-muted'
                        : 'text-muted-foreground',
                    'disabled:cursor-default',
                  )}
                >
                  <span
                    className={cn(
                      'flex size-[22px] shrink-0 items-center justify-center rounded-full border-[1.5px] font-mono text-[11px] font-semibold transition-colors duration-200',
                      done
                        ? 'border-primary bg-primary text-primary-foreground'
                        : current
                          ? 'border-foreground'
                          : 'border-border',
                    )}
                  >
                    {done ? (
                      <Check className="size-3" strokeWidth={3} aria-hidden />
                    ) : (
                      position + 1
                    )}
                  </span>
                  {t(`onboarding.steps.${id}.label`)}
                  {done && (
                    <span className="sr-only">
                      {' '}
                      ({t('onboarding.stepDone')})
                    </span>
                  )}
                </button>
              </li>
            )
          })}
        </ol>
      </nav>
      <div className="flex-1" {...backdropDragRegion()} />
      <AppearanceControls />
    </aside>
  )
}

function AppearanceControls() {
  const { t, preference, setPreference, language } = useI18n()
  const { resolved, setPreference: setTheme } = useTheme()
  return (
    <div className="flex gap-1.5">
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            variant="outline"
            size="sm"
            aria-label={t('shell.nav.language')}
            className="h-[30px] rounded-lg bg-transparent px-2.5 text-xs font-medium text-muted-foreground shadow-none"
          >
            {languageNames[language]}
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent side="top" align="start">
          <DropdownMenuRadioGroup
            value={preference}
            onValueChange={(value) =>
              setPreference(value as LanguagePreference)
            }
          >
            <DropdownMenuRadioItem value="system">
              {t('shell.theme.system')}
            </DropdownMenuRadioItem>
            {supportedLanguages.map((lang) => (
              <DropdownMenuRadioItem key={lang} value={lang}>
                {languageNames[lang]}
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>
        </DropdownMenuContent>
      </DropdownMenu>
      <Button
        variant="outline"
        size="sm"
        aria-label={t(
          resolved === 'dark'
            ? 'shell.nav.themeToLight'
            : 'shell.nav.themeToDark',
        )}
        onClick={() => setTheme(resolved === 'dark' ? 'light' : 'dark')}
        className="h-[30px] w-8 rounded-lg bg-transparent px-0 text-muted-foreground shadow-none"
      >
        {resolved === 'dark' ? <Sun /> : <Moon />}
      </Button>
    </div>
  )
}
