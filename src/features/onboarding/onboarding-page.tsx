/**
 * First-run setup: seven steps from welcome to the first backup, then hand
 * off to the main app.
 *
 * Responsible for: the window layout (step list, progress bar, language and
 * theme), moving between steps, holding the draft, and saving the browser
 * choice so an interrupted setup resumes with it.
 * Not responsible for: what each step shows (the `*-step.tsx` files) or
 * creating the archive (see `use-finish-setup.ts`).
 */
import { useQueryClient } from '@tanstack/react-query'
import { ArrowLeft, ArrowRight, Check, Moon, Sun } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useSession } from '@/app/session'
import { BrandMark } from '@/components/app/brand-mark'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Spinner } from '@/components/ui/spinner'
import { cn } from '@/lib/cn'
import { describeError } from '@/lib/errors'
import { languageNames, supportedLanguages, useI18n } from '@/lib/i18n'
import { currentSnapshot, useSaveConfig, useSnapshot } from '@/lib/queries/app'
import { hasMacOverlayTitlebar, isMacOsHost } from '@/lib/runtime'
import { useTheme } from '@/lib/theme'
import type { LanguagePreference } from '@/lib/types'
import { AiStep } from './ai-step'
import { BrowsersStep } from './browsers-step'
import { DoneStep } from './done-step'
import {
  backgroundCanUnlock,
  canContinue,
  initialDraft,
  steps,
  type Draft,
} from './draft'
import { EncryptionStep } from './encryption-step'
import { InlineError } from './inline-error'
import { ScheduleStep } from './schedule-step'
import { useSchedulePreview } from './use-schedule-preview'
import { StorageStep } from './storage-step'
import { stagesFor, useFinishSetup } from './use-finish-setup'
import { WelcomeStep } from './welcome-step'

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
  const step = steps[index]
  const where = t(
    isMacOsHost() ? 'onboarding.where.mac' : 'onboarding.where.other',
  )

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
  const finish = useFinishSetup(draft, stages)
  const locked = finish.state.status !== 'idle'
  const ready = canContinue(step, draft, snapshot)

  const update = (patch: Partial<Draft>) =>
    setDraft((current) => ({ ...current, ...patch }))

  function go(next: number) {
    setSaveError(null)
    setIndex(next)
  }

  async function next() {
    if (!ready) return
    if (step === 'browsers') {
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
    if (step !== 'done') return go(index + 1)
    if (finish.state.status === 'done')
      return session.enter(currentSnapshot(client))
    if (finish.state.status === 'idle') finish.start()
  }

  function skip() {
    update(step === 'schedule' ? { frequency: 'off' } : { ai: 'off' })
    go(index + 1)
  }

  return (
    <div className="flex h-full bg-window backdrop-blur-[40px] backdrop-saturate-[1.4]">
      <StepList
        index={index}
        locked={locked}
        onSelect={go}
        overlayTitlebar={hasMacOverlayTitlebar()}
      />
      <main className="my-2.5 mr-2.5 flex min-w-0 flex-1 flex-col overflow-hidden rounded-xl border bg-panel">
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
        <StepScroller index={index}>
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
            </p>
          </header>

          {step === 'welcome' && <WelcomeStep />}
          {step === 'browsers' && (
            <BrowsersStep
              selected={draft.selectedProfileIds}
              onChange={(selectedProfileIds) => update({ selectedProfileIds })}
              showHint={!ready}
            />
          )}
          {step === 'storage' && (
            <StorageStep selected={draft.selectedProfileIds} />
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
          {step === 'done' && (
            <DoneStep
              draft={draft}
              stages={stages}
              finish={finish.state}
              onRetry={finish.start}
              onSkip={() => void finish.skip()}
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
  children,
}: {
  index: number
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
        className="m-auto flex w-full max-w-[620px] animate-rise flex-col gap-6 px-10 py-12 motion-reduce:animate-none"
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
      {overlayTitlebar ? (
        <div data-tauri-drag-region className="h-[52px] shrink-0" />
      ) : (
        <div className="h-7 shrink-0" />
      )}
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
      <div className="flex-1" data-tauri-drag-region />
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
