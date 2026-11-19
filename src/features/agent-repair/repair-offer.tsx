/**
 * What a move or an encryption change says when the backend refused it
 * because saved Agent conversations can't be read, with the way out:
 * - damaged (`conversation-repair-required`): why, that nothing changed, and
 *   "Repair conversations…", which closes the dialog it is in and opens the
 *   repair over the same screen;
 * - not readable for now (`conversation-access-unavailable`): the system
 *   refuses access, which is not damage, so "Try again" asks the backend to
 *   open the files again (`retry_agent_store`); once they open, the dialog's
 *   own button starts the operation again.
 * Both show the file the refusal names, when it names one.
 */
import { RotateCw, Wrench } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Spinner } from '@/components/ui/spinner'
import { useRetryStore } from '@/features/data-kinds/use-retry-store'
import { AffectedFile } from '@/features/settings/affected-file'
import { useI18n } from '@/lib/i18n'
import { useAgentKind } from '@/lib/queries/agent-backup'
import { useOpenAgentRepair } from './links'

type Operation = 'export' | 'import' | 'encryption'

const boxClass =
  'flex flex-col gap-2 rounded-lg border bg-card px-3.5 py-3 text-[13px]'

export function RepairOffer({
  operation,
  error,
  onLeave,
}: {
  operation: Operation
  /** The refusal, for the file it names. */
  error: unknown
  /** Closes the dialog the offer is in, before the repair opens. */
  onLeave: () => void
}) {
  const { t } = useI18n()
  const openRepair = useOpenAgentRepair()
  return (
    <div role="alert" data-repair-offer={operation} className={boxClass}>
      <p className="[overflow-wrap:anywhere]">
        {t(`agentRepair.required.${operation}`)}
      </p>
      <AffectedFile error={error} />
      <Button
        type="button"
        size="sm"
        variant="outline"
        className="self-start"
        onClick={() => {
          onLeave()
          openRepair()
        }}
      >
        <Wrench aria-hidden />
        {t('agentRepair.open')}
      </Button>
    </div>
  )
}

export function AccessOffer({
  operation,
  error,
}: {
  operation: Operation
  /** The refusal, for the file it names. */
  error: unknown
}) {
  const { t } = useI18n()
  const retry = useRetryStore()
  const { row } = useAgentKind()
  // Only a retry asked for here can say the files open again: until then the
  // list may still be the one from before the refusal.
  const [retried, setRetried] = useState(false)
  const readable = retried && row?.availability === 'available'
  return (
    <div role="alert" data-access-offer={operation} className={boxClass}>
      <p className="[overflow-wrap:anywhere]">
        {t(`agentRepair.access.${operation}`)}
      </p>
      {readable ? (
        <p role="status" className="text-muted-foreground">
          {t('agentRepair.access.readable')}
        </p>
      ) : (
        <>
          <AffectedFile error={error} />
          <p className="text-muted-foreground">
            {t(
              retried
                ? 'agentRepair.access.stillRefused'
                : 'agentRepair.access.hint',
            )}
          </p>
          <Button
            type="button"
            size="sm"
            variant="outline"
            className="self-start"
            disabled={retry.busy}
            onClick={() =>
              void retry.retry().then(() => {
                setRetried(true)
              })
            }
          >
            {retry.busy ? <Spinner /> : <RotateCw aria-hidden />}
            {t('dataKinds.damage.retry')}
          </Button>
        </>
      )}
    </div>
  )
}
