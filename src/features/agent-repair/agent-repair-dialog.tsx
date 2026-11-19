/**
 * Repairing saved Agent conversations (`?repair=agent` over any screen,
 * `agent-history.md` › 從安全副本修復對話): Preview → Execute.
 *
 * On opening it checks at once what can be repaired (reading only; browsing
 * keeps working). The backend picks the repair: rebuild the lists when the
 * saved conversations are intact; otherwise start over with what still
 * checks out, or, if the person chooses one, bring them back from a safety
 * copy. The plan says what is wrong, what the repair does, the counts and
 * where what it replaces is kept; "Repair" installs exactly what was checked.
 * Every way of closing lets go of a check the backend holds.
 *
 * Responsibilities: the layout per step and the buttons. Not responsible
 * for: the sequence (`use-agent-repair.ts`), the plan's words
 * (`repair-plan.tsx`), the sources (`repair-sources.tsx`), the result
 * (`repair-result.tsx`).
 */
import { useEffect, useRef, type RefObject } from 'react'
import { useNavigate } from 'react-router-dom'
import { MaintenanceProgress } from '@/components/app/maintenance-progress'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Spinner } from '@/components/ui/spinner'
import { damageWords } from '@/features/settings/kinds/kind-damage'
import { maintenanceClient } from '@/lib/backend-client/maintenance'
import { useI18n } from '@/lib/i18n'
import { useAgentKind } from '@/lib/queries/agent-backup'
import { spaceBackupPath } from '@/lib/spaces'
import { useAgentRepairParam } from './links'
import { startOverLeavesSomething } from './repair-counts'
import { repairErrorIs, repairErrorText } from './repair-errors'
import { RepairPlan } from './repair-plan'
import { RepairResult } from './repair-result'
import { RepairSources } from './repair-sources'
import {
  LOCAL,
  sameSource,
  useAgentRepair,
  type AgentRepairFlow,
} from './use-agent-repair'

/** What the backend reported about the conversation files, while they don't open. */
function WhatsWrong() {
  const { t } = useI18n()
  const { row } = useAgentKind()
  if (row?.availability !== 'unavailable') return null
  return (
    <section className="flex flex-col gap-0.5 text-[13px]">
      <h3 className="text-sm font-medium">{t('agentRepair.whatsWrong')}</h3>
      <p className="text-muted-foreground">
        {damageWords(row.failure)
          .map((key) => t(key))
          .join(' ')}
      </p>
    </section>
  )
}

function Checking({ flow }: { flow: AgentRepairFlow }) {
  const { t } = useI18n()
  return (
    <div className="flex flex-col gap-1.5 text-[13px]">
      <p className="font-medium">{t('agentRepair.checking')}</p>
      <MaintenanceProgress onStop={flow.markStopping} />
      <p className="text-xs text-muted-foreground">
        {t('agentRepair.checkingHint')}
      </p>
    </div>
  )
}

/** The main button's next step: repair what was checked, or check the chosen source. */
function primaryLabel(
  flow: AgentRepairFlow,
  t: ReturnType<typeof useI18n>['t'],
) {
  if (flow.prepared) return t(`agentRepair.actions.${flow.prepared.action}.run`)
  if (flow.chosen.kind === 'copy') return t('agentRepair.sources.check')
  return flow.canChoose
    ? t('agentRepair.sources.checkLocal')
    : t('agentRepair.errors.checkAgain')
}

function RepairFlow({
  holdsRef,
  onClose,
}: {
  holdsRef: RefObject<boolean>
  onClose: () => void
}) {
  const { t } = useI18n()
  const navigate = useNavigate()
  const flow = useAgentRepair()
  const { check, phase } = flow
  const started = useRef(false)

  // Opening the dialog is the request to check. Development renders effects
  // twice; the ref keeps that to one check, which nothing cancels on its own.
  useEffect(() => {
    if (started.current) return
    started.current = true
    void check(LOCAL)
  }, [check])

  useEffect(() => {
    holdsRef.current = flow.holds
  }, [flow.holds, holdsRef])

  const running = phase === 'running'
  const busy = phase === 'checking' || running
  const failure =
    flow.failure &&
    (flow.failure.during === 'repair' ||
      sameSource(flow.failure.source, flow.chosen))
      ? flow.failure
      : null
  const healthy =
    failure !== null &&
    repairErrorIs(failure.error, 'conversation-repair-healthy')
  const passwordMissing =
    flow.chosen.kind === 'copy' &&
    flow.asked.has(flow.chosen.path) &&
    !flow.passwords[flow.chosen.path]
  const destructive =
    flow.prepared !== null && flow.prepared.action !== 'rebuild'

  const primary = () => {
    if (flow.prepared) void flow.repair()
    else void check(flow.chosen)
  }

  return (
    <DialogContent
      className="flex max-h-[min(88vh,760px)] flex-col sm:max-w-lg"
      showCloseButton={!running}
      onInteractOutside={(event) => running && event.preventDefault()}
      onEscapeKeyDown={(event) => running && event.preventDefault()}
    >
      <DialogHeader>
        <DialogTitle>{t('agentRepair.title')}</DialogTitle>
        <DialogDescription>{t('agentRepair.intro')}</DialogDescription>
      </DialogHeader>

      <div className="-mx-1 flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-1">
        {phase === 'done' && flow.done ? (
          <RepairResult done={flow.done} />
        ) : (
          <>
            <WhatsWrong />
            {phase === 'checking' && <Checking flow={flow} />}
            {flow.canChoose && (
              <RepairSources
                chosen={flow.chosen}
                onChoose={flow.choose}
                disabled={busy}
                suggestCopies={
                  flow.prepared !== null &&
                  startOverLeavesSomething(flow.prepared)
                }
                asked={flow.asked}
                passwords={flow.passwords}
                onPassword={flow.setPassword}
              />
            )}
            {phase === 'ready' && flow.prepared && (
              <RepairPlan preview={flow.prepared} />
            )}
            {running && <MaintenanceProgress onStop={flow.markStopping} />}
            {flow.stopped && (
              <p role="status" className="text-[13px] text-muted-foreground">
                {t('maintenance.stopped')}
              </p>
            )}
            {failure && phase !== 'checking' && (
              <p
                role="alert"
                data-repair-error
                className="text-[13px] [overflow-wrap:anywhere] text-destructive"
              >
                {repairErrorText(t, failure.error, failure.during)}
              </p>
            )}
          </>
        )}
      </div>

      <DialogFooter>
        {phase === 'done' ? (
          <>
            {flow.done?.preview.reviewSourcesBeforeBackup && (
              <Button
                variant="outline"
                onClick={() => void navigate(spaceBackupPath('agent'))}
              >
                {t('agentRepair.done.openBackup')}
              </Button>
            )}
            <Button onClick={onClose}>{t('agentRepair.done.close')}</Button>
          </>
        ) : healthy ? (
          <Button onClick={onClose}>{t('common.close')}</Button>
        ) : (
          <>
            <Button variant="ghost" disabled={running} onClick={onClose}>
              {t('common.cancel')}
            </Button>
            {/* While a check runs its progress has the Stop; the next step comes after it. */}
            {phase !== 'checking' && (
              <Button
                variant={destructive ? 'destructive' : 'default'}
                disabled={running || (!flow.prepared && passwordMissing)}
                onClick={primary}
              >
                {running && <Spinner />}
                {primaryLabel(flow, t)}
              </Button>
            )}
          </>
        )}
      </DialogFooter>
    </DialogContent>
  )
}

/**
 * The dialog over the current screen. However it closes (its buttons, Esc,
 * or the address changing under it), a check the backend still holds or is
 * still running is let go (`cancel_maintenance`), so it doesn't wait out its
 * 15 minutes or keep other archive changes waiting.
 */
export default function AgentRepairDialog() {
  const { open, close } = useAgentRepairParam()
  const holdsRef = useRef(false)
  useEffect(() => {
    if (open || !holdsRef.current) return
    holdsRef.current = false
    void maintenanceClient.cancel().catch(() => undefined)
  }, [open])
  return (
    <Dialog open={open} onOpenChange={(next) => !next && close()}>
      {open && <RepairFlow holdsRef={holdsRef} onClose={close} />}
    </Dialog>
  )
}
