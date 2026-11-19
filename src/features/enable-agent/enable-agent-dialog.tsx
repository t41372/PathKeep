/**
 * Turning Agent conversations on: Preview → Choose → Save, in a dialog
 * opened by `?enable=agent` over whatever screen is open
 * (`data-spaces.md` §3.2). Nothing is read before the person opens it, and
 * the preview only counts files (number, size, dates); it never opens a
 * conversation.
 *
 * Responsibilities: the steps and their buttons, starting the preview while
 * the dialog is open and the kind is off, and closing the dialog when the
 * kind turns out to be on already. Every opening starts from the preview
 * with a fresh walk; the choices made in the choose step are kept as device
 * settings, so they come back.
 *
 * Not in this build: the save step. Its button is there, disabled, and
 * says why; the backend's enable command comes with the next step.
 */
import { useEffect, useId, useMemo, useState } from 'react'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { useI18n } from '@/lib/i18n'
import { useDataKinds } from '@/lib/queries/data-kinds'
import { kindStateOf } from '@/lib/spaces'
import { ChooseStep } from './choose-step'
import { useEnableAgentParam } from './links'
import { toolChoices } from './preview-model'
import { PreviewStep } from './preview-step'
import { Steps } from './steps'
import { useAgentChoices } from './use-agent-choices'
import { useAgentPreview } from './use-agent-preview'

function EnableFlow({
  active,
  onClose,
}: {
  active: boolean
  onClose: () => void
}) {
  const { t } = useI18n()
  const reasonId = useId()
  const [step, setStep] = useState<0 | 1>(0)
  const preview = useAgentPreview(active)
  const report =
    preview.state.phase === 'finished' ? preview.state.report : null
  const expired = preview.state.phase === 'finished' && preview.state.expired
  const tools = useMemo(() => (report ? toolChoices(report) : []), [report])
  const choices = useAgentChoices(tools)
  const anyTicked = tools.some(
    (tool) => tool.readable && choices.ticked.has(tool.tool),
  )

  const rerun = () => {
    setStep(0)
    preview.rerun()
  }

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
          <PreviewStep
            state={preview.state}
            onCancel={preview.cancel}
            onRerun={rerun}
          />
        ) : (
          <div className="flex flex-col gap-4">
            {expired && (
              <div
                role="status"
                className="flex flex-wrap items-center justify-between gap-3 rounded-lg border bg-card px-3.5 py-3 text-[13px]"
              >
                {t('enableAgent.preview.expired')}
                <Button variant="outline" size="sm" onClick={rerun}>
                  {t('enableAgent.preview.runAgain')}
                </Button>
              </div>
            )}
            <ChooseStep tools={tools} choices={choices} />
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
            <p
              id={reasonId}
              className="text-xs text-muted-foreground sm:mr-auto sm:max-w-[420px]"
            >
              {t(
                anyTicked
                  ? 'enableAgent.choose.notConnected'
                  : 'enableAgent.choose.needOne',
              )}
            </p>
            <Button variant="ghost" onClick={() => setStep(0)}>
              {t('enableAgent.actions.back')}
            </Button>
            {/* The save step's backend lands next; until then nothing can be saved. */}
            <Button disabled aria-describedby={reasonId}>
              {t('enableAgent.actions.start')}
            </Button>
          </>
        )}
      </DialogFooter>
    </>
  )
}

export default function EnableAgentDialog() {
  const { open, close } = useEnableAgentParam()
  const kinds = useDataKinds()
  const state = kindStateOf(kinds.data, 'agent-history')
  // Each opening is a fresh flow; the old one stays while the dialog fades.
  const [session, setSession] = useState(0)
  const [wasOpen, setWasOpen] = useState(false)
  if (open !== wasOpen) {
    setWasOpen(open)
    if (open) setSession((count) => count + 1)
  }

  // Only a kind that is off is turned on here; a stale link closes.
  const alreadyOn = state !== undefined && state !== 'off'
  useEffect(() => {
    if (open && alreadyOn) close()
  }, [open, alreadyOn, close])

  return (
    <Dialog open={open && !alreadyOn} onOpenChange={(next) => !next && close()}>
      <DialogContent className="gap-5 sm:max-w-[760px]">
        <EnableFlow
          key={session}
          // Without an answer about the kind, the preview's own refusal says why.
          active={open && (state === 'off' || (!state && kinds.isError))}
          onClose={close}
        />
      </DialogContent>
    </Dialog>
  )
}
