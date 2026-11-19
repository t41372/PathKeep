/**
 * What a move or an encryption change says when the backend refused it
 * because saved Agent conversations can't be read
 * (`conversation-repair-required`): why, that nothing changed, and the way
 * to the repair. The button closes the dialog it is in and opens the repair
 * over the same screen.
 */
import { Wrench } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { useI18n } from '@/lib/i18n'
import { useOpenAgentRepair } from './links'

export function RepairOffer({
  operation,
  onLeave,
}: {
  operation: 'export' | 'import' | 'encryption'
  /** Closes the dialog the offer is in, before the repair opens. */
  onLeave: () => void
}) {
  const { t } = useI18n()
  const openRepair = useOpenAgentRepair()
  return (
    <div
      role="alert"
      data-repair-offer={operation}
      className="flex flex-col gap-2 rounded-lg border bg-card px-3.5 py-3 text-[13px]"
    >
      <p className="[overflow-wrap:anywhere]">
        {t(`agentRepair.required.${operation}`)}
      </p>
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
