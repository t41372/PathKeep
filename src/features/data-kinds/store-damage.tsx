/**
 * What a kind whose own storage will not open says, wherever it shows (its
 * card in Manage data types, its backup page): which file is damaged or
 * cannot be read, when that was found, that browsing history is not
 * affected, "Try again", and for Agent conversations "Repair
 * conversations…" when a repair can help.
 *
 * Responsibilities: the words (`kind-damage.ts` picks them) and the
 * buttons. Not responsible for: the retry itself (`use-retry-store.ts`) or
 * the repair (`features/agent-repair/`, opened over the current screen).
 */
import { CircleAlert } from 'lucide-react'
import { Button } from '@/components/ui/button'
import type { DataKindRow } from '@/lib/backend-client/data-kinds'
import { useI18n } from '@/lib/i18n'
import type { DataSpace } from '@/lib/spaces'
import { useOpenAgentRepair } from '@/features/agent-repair/links'
import { canRepair, damageWords } from '@/features/settings/kinds/kind-damage'

export function StoreDamage({
  space,
  row,
  onRetry,
  busy = false,
}: {
  space: DataSpace
  row: DataKindRow
  onRetry: () => void
  busy?: boolean
}) {
  const { t } = useI18n()
  const openRepair = useOpenAgentRepair()
  const repairable = row.kind === 'agent-history' && canRepair(row.failure)
  return (
    <div
      role="alert"
      data-store-damage={row.kind}
      className="flex flex-wrap gap-2 rounded-lg border bg-background/60 px-3 py-2.5 text-[13px]"
    >
      <CircleAlert
        aria-hidden
        className="mt-0.5 size-4 shrink-0 text-destructive"
      />
      <div className="flex min-w-0 flex-1 basis-60 flex-col gap-0.5">
        <span className="font-medium">
          {t('dataKinds.damage.title', { name: t(space.name) })}
        </span>
        <span className="text-muted-foreground">
          {[
            ...damageWords(row.failure),
            'dataKinds.damage.notAffected' as const,
          ]
            .map((key) => t(key))
            .join(' ')}
        </span>
      </div>
      <div className="flex shrink-0 items-start gap-2 self-start">
        <Button variant="outline" size="sm" disabled={busy} onClick={onRetry}>
          {t('dataKinds.damage.retry')}
        </Button>
        {repairable && (
          <Button size="sm" disabled={busy} onClick={openRepair}>
            {t('agentRepair.open')}
          </Button>
        )}
      </div>
    </div>
  )
}
