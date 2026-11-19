/**
 * Some saved conversations can't be read while the rest open: their lists
 * were found damaged and set aside, one conversation at a time
 * (`readerRebuildRequired` on the available Agent-history row). Not the
 * damage notice: the store works, nothing is lost, and browsing and the
 * other conversations go on as usual. Shown on the Agent backup page and
 * on the Agent card in Manage data types, with "Repair conversations…",
 * which opens the repair over the same screen; the repair's check rebuilds
 * the lists of those conversations. It goes away when the row says the
 * lists are whole again (the kinds-changed event refetches the row).
 *
 * Not responsible for: the repair (`agent-repair-dialog.tsx`).
 */
import { Info } from 'lucide-react'
import { Button } from '@/components/ui/button'
import type { DataKindRow } from '@/lib/backend-client/data-kinds'
import { useI18n } from '@/lib/i18n'
import { useOpenAgentRepair } from './links'

/** Nothing unless the row is the available Agent-history row that needs a rebuild. */
export function ParkedNotice({ row }: { row: DataKindRow | undefined }) {
  const { t } = useI18n()
  const openRepair = useOpenAgentRepair()
  if (
    row?.kind !== 'agent-history' ||
    row.availability !== 'available' ||
    row.readerRebuildRequired !== true
  )
    return null
  return (
    <div
      role="status"
      data-parked-notice
      className="flex animate-in flex-wrap items-start gap-3 rounded-xl border bg-card px-4 py-3 text-[13px] duration-200 fade-in-0 slide-in-from-top-1"
    >
      <Info
        aria-hidden
        className="mt-0.5 size-4 shrink-0 text-muted-foreground"
      />
      <div className="flex min-w-0 flex-1 basis-60 flex-col gap-0.5">
        <span className="font-medium">{t('agentRepair.parked.title')}</span>
        <span className="text-muted-foreground">
          {t('agentRepair.parked.body')}
        </span>
      </div>
      <Button
        size="sm"
        variant="outline"
        className="shrink-0 self-start"
        onClick={openRepair}
      >
        {t('agentRepair.open')}
      </Button>
    </div>
  )
}
