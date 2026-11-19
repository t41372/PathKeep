/**
 * Turning Agent conversations on: Preview → Choose → Save, in a dialog
 * opened by `?enable=agent` over whatever screen is open
 * (`data-spaces.md` §3.2). Nothing is read before the person opens it, and
 * the preview only counts files (number, size, dates); it never opens a
 * conversation.
 *
 * Responsibilities: the steps and their buttons; starting the preview while
 * the dialog is open and the kind is off (or only visible because the
 * archive holds conversations from elsewhere); starting the first save,
 * which turns the kind on at once; showing that save as it runs and once it
 * ended, also when the dialog is closed and opened again; and closing an
 * old link when the kind is on and no first save is there to show. Every
 * opening before the save starts from the preview with a fresh walk; the
 * choices made in the choose step are device settings, so they come back.
 *
 * Closing the dialog, in any way, never stops the save: it goes on as a
 * background task.
 */
import { useCallback, useEffect, useId, useMemo, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { isStoreDamage } from '@/lib/backend-client/data-kinds'
import { describeError } from '@/lib/errors'
import { useI18n, type MessageKey } from '@/lib/i18n'
import { useAgentKind } from '@/lib/queries/agent-backup'
import { spaceStartPath, useSpacesOn } from '@/lib/spaces'
import { ChooseStep } from './choose-step'
import {
  forgetFirstSave,
  useFirstSaveToShow,
  type ShownSave,
} from './first-save'
import { useEnableAgentParam } from './links'
import { toolChoices } from './preview-model'
import { PreviewStep } from './preview-step'
import { EndedTitle, SaveStep } from './save-step'
import { Steps } from './steps'
import { useAgentChoices } from './use-agent-choices'
import { useAgentPreview } from './use-agent-preview'
import { useShownRun } from './use-shown-run'
import { useStartSaving, type RestartReason } from './use-start-saving'

const restartWords: Record<RestartReason, MessageKey> = {
  expired: 'enableAgent.restart.expired',
  changed: 'enableAgent.restart.changed',
  policy: 'enableAgent.restart.policy',
  empty: 'enableAgent.restart.empty',
}

/** Preview and Choose, until the save starts. */
function ChooseFlow({
  active,
  onClose,
}: {
  active: boolean
  onClose: () => void
}) {
  const { t } = useI18n()
  const reasonId = useId()
  const [step, setStep] = useState<0 | 1>(0)
  const [restart, setRestart] = useState<RestartReason | null>(null)
  const [startError, setStartError] = useState<unknown>(null)
  const preview = useAgentPreview(active)
  const report =
    preview.state.phase === 'finished' ? preview.state.report : null
  const expired = preview.state.phase === 'finished' && preview.state.expired
  const tools = useMemo(() => (report ? toolChoices(report) : []), [report])
  const choices = useAgentChoices(tools)
  const saving = useStartSaving()
  const anyTicked = tools.some(
    (tool) => tool.readable && choices.ticked.has(tool.tool),
  )

  const rerun = (reason: RestartReason | null = null) => {
    setStep(0)
    setRestart(reason)
    setStartError(null)
    preview.rerun()
  }

  const start = async () => {
    if (!report) return
    setStartError(null)
    const outcome = await saving.start({
      report,
      ticked: choices.ticked,
      readGit: choices.readGit,
      flush: choices.flush,
    })
    switch (outcome.kind) {
      case 'started':
        // The dialog follows the remembered save from here.
        return
      case 'restart':
        rerun(outcome.reason)
        return
      case 'elsewhere':
        toast(t('enableAgent.choose.turnedOnElsewhere'))
        onClose()
        return
      case 'failed':
        setStartError(outcome.error)
    }
  }

  const reason = !anyTicked
    ? t('enableAgent.choose.needOne')
    : saving.starting
      ? t('enableAgent.choose.starting')
      : null

  return (
    <>
      <DialogHeader className="gap-3">
        <Steps at={step} />
        <DialogTitle className="text-lg">
          {t(
            step === 0
              ? 'enableAgent.preview.title'
              : 'enableAgent.choose.title',
          )}
        </DialogTitle>
        <DialogDescription>
          {t(
            step === 0 ? 'enableAgent.preview.body' : 'enableAgent.choose.body',
          )}
        </DialogDescription>
      </DialogHeader>
      <div key={step} className="animate-rise">
        {step === 0 ? (
          <div className="flex flex-col gap-4">
            {restart && (
              <p
                role="status"
                className="rounded-lg border bg-card px-3.5 py-3 text-[13px]"
              >
                {t(restartWords[restart])}
              </p>
            )}
            <PreviewStep
              state={preview.state}
              onCancel={preview.cancel}
              onRerun={() => rerun()}
            />
          </div>
        ) : (
          <div className="flex flex-col gap-4">
            {expired && (
              <div
                role="status"
                className="flex flex-wrap items-center justify-between gap-3 rounded-lg border bg-card px-3.5 py-3 text-[13px]"
              >
                {t('enableAgent.preview.expired')}
                <Button variant="outline" size="sm" onClick={() => rerun()}>
                  {t('enableAgent.preview.runAgain')}
                </Button>
              </div>
            )}
            <ChooseStep tools={tools} choices={choices} />
            {startError !== null && (
              <div
                role="alert"
                className="flex flex-col gap-1 rounded-lg border bg-card px-3.5 py-3 text-[13px]"
              >
                <span className="font-medium">
                  {t('enableAgent.choose.startFailed')}
                </span>
                <span className="text-xs [overflow-wrap:anywhere] text-muted-foreground">
                  {isStoreDamage(startError)
                    ? t('enableAgent.preview.reasons.unavailable')
                    : describeError(startError, 'enable_agent_history')}
                </span>
              </div>
            )}
          </div>
        )}
      </div>
      <DialogFooter className="items-center gap-2 sm:gap-2">
        {step === 0 ? (
          <>
            <Button variant="ghost" onClick={onClose}>
              {t('enableAgent.actions.noThanks')}
            </Button>
            <Button
              disabled={report === null || tools.length === 0}
              onClick={() => setStep(1)}
            >
              {t('enableAgent.actions.next')}
            </Button>
          </>
        ) : (
          <>
            {reason && (
              <p
                id={reasonId}
                className="text-xs text-muted-foreground sm:mr-auto sm:max-w-[420px]"
              >
                {reason}
              </p>
            )}
            <Button
              variant="ghost"
              disabled={saving.starting}
              onClick={() => setStep(0)}
            >
              {t('enableAgent.actions.back')}
            </Button>
            <Button
              disabled={!anyTicked || expired || saving.starting}
              aria-describedby={reason ? reasonId : undefined}
              onClick={() => void start()}
            >
              {t('enableAgent.actions.start')}
            </Button>
          </>
        )}
      </DialogFooter>
    </>
  )
}

/** The save, from its first progress to how it ended. */
function SaveFlow({
  save,
  onClose,
  onEnd,
}: {
  save: ShownSave
  onClose: (seenEnd: boolean) => void
  /** Whether the end is on screen, so closing in any way counts as seen. */
  onEnd: (shown: boolean) => void
}) {
  const { t } = useI18n()
  const navigate = useNavigate()
  const shown = useShownRun(save)
  const on = useSpacesOn()
  const agent = on.find((space) => space.id === 'agent')
  const done = shown.finished
  useEffect(() => onEnd(done), [done, onEnd])

  return (
    <>
      <DialogHeader className="gap-3">
        <Steps at={done ? 3 : 2} />
        <DialogTitle className="text-lg">
          {done ? (
            <EndedTitle result={shown.detail?.result} />
          ) : (
            t('enableAgent.run.title')
          )}
        </DialogTitle>
        <DialogDescription className={done ? 'sr-only' : undefined}>
          {t('enableAgent.run.body')}
        </DialogDescription>
      </DialogHeader>
      <div key={done ? 'done' : 'run'} className="animate-rise">
        <SaveStep shown={shown} save={save} />
      </div>
      <DialogFooter className="items-center gap-2 sm:gap-2">
        {done ? (
          <>
            <Button variant="ghost" onClick={() => onClose(true)}>
              {t('common.close')}
            </Button>
            <Button
              disabled={!agent}
              onClick={() => {
                onClose(true)
                if (agent) void navigate(spaceStartPath(agent))
              }}
            >
              {t('enableAgent.done.view')}
            </Button>
          </>
        ) : (
          <Button
            variant="outline"
            onClick={() => {
              onClose(false)
              toast(t('enableAgent.run.continued'), {
                description: t('enableAgent.run.continuedBody'),
              })
            }}
          >
            {t('enableAgent.run.continue')}
          </Button>
        )}
      </DialogFooter>
    </>
  )
}

export default function EnableAgentDialog() {
  const { open, close } = useEnableAgentParam()
  const { kinds, row, viewOnly } = useAgentKind()
  const state = row?.state
  const first = useFirstSaveToShow()
  // Each opening is a fresh flow; the old one stays while the dialog fades.
  const [session, setSession] = useState(0)
  const [wasOpen, setWasOpen] = useState(false)
  if (open !== wasOpen) {
    setWasOpen(open)
    if (open) setSession((count) => count + 1)
  }

  const turningOn = state === 'off' || viewOnly
  // The first save turns the kind on; off again since, a remembered one is
  // over, and this opening turns it on anew.
  const save = turningOn ? null : first.save
  const forget = turningOn && first.remembered
  useEffect(() => {
    if (forget) forgetFirstSave()
  }, [forget])
  // On here already, and no first save to show: an old link, which closes.
  const stale =
    state !== undefined && !turningOn && first.known && save === null
  useEffect(() => {
    if (open && stale) close()
  }, [open, stale, close])

  const shown =
    open && !stale && (turningOn || save !== null || (!state && kinds.isError))

  const endShown = useRef(false)
  const onEnd = useCallback((value: boolean) => {
    endShown.current = value
  }, [])
  const closeDialog = (seenEnd = endShown.current) => {
    if (seenEnd) forgetFirstSave()
    close()
  }

  return (
    <Dialog
      open={shown}
      onOpenChange={(next) => {
        if (!next) closeDialog()
      }}
    >
      <DialogContent className="gap-5 sm:max-w-[760px]">
        {save ? (
          <SaveFlow
            key={save.taskId}
            save={save}
            onClose={closeDialog}
            onEnd={onEnd}
          />
        ) : (
          <ChooseFlow
            key={session}
            // Without an answer about the kind, the preview's own refusal says why.
            active={open && (turningOn || (!state && kinds.isError))}
            onClose={() => closeDialog(false)}
          />
        )}
      </DialogContent>
    </Dialog>
  )
}
