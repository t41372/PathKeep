/**
 * What a kind whose own storage will not open says, wherever it shows (its
 * card in Manage data types, its backup page): which file is damaged or
 * cannot be read, when that was found, that browsing history is not
 * affected, and "Try again".
 *
 * Responsibilities: the words (`kind-damage.ts` picks them) and the
 * button. Not responsible for: the retry itself (`use-retry-store.ts`).
 */
import { CircleAlert } from 'lucide-react'
import { Button } from '@/components/ui/button'
import type { DataKindRow } from '@/lib/backend-client/data-kinds'
import { useI18n } from '@/lib/i18n'
import type { DataSpace } from '@/lib/spaces'
import { damageWords } from '@/features/settings/kinds/kind-damage'

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
  return (
    <div
      role="alert"
      className="flex gap-2 rounded-lg border bg-background/60 px-3 py-2.5 text-[13px]"
    >
      <CircleAlert
        aria-hidden
        className="mt-0.5 size-4 shrink-0 text-destructive"
      />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
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
      <Button
        variant="outline"
        size="sm"
        className="shrink-0 self-start"
        disabled={busy}
        onClick={onRetry}
      >
        {t('dataKinds.damage.retry')}
      </Button>
    </div>
  )
}
