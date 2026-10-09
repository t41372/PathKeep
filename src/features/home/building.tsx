/**
 * Home's Insights cards while the background build has not produced their data yet: the
 * section reports `stale` and holds nothing. Then the card keeps its skeleton and says what is
 * happening, instead of an empty chart or "nothing here yet", and its query keeps polling
 * (`pollWhileStale`) until the build is done. A section that was never computed is a first
 * build (a new archive, Insights rebuilt from scratch); one computed before is being updated
 * (after a backup, or rebuilt with the rules of a new version).
 *
 * Responsible for: that line. Not responsible for: telling a still-building section from an
 * empty one (`stillBuilding` in home-data.ts), the cards' skeletons, or queries.
 */
import { Spinner } from '@/components/ui/spinner'
import type { Section } from '@/lib/backend-client/insights'
import { useI18n } from '@/lib/i18n'

export function BuildingNote({
  result,
}: {
  result: Section<unknown> | undefined
}) {
  const { t } = useI18n()
  return (
    <p
      role="status"
      className="flex items-center gap-1.5 text-xs text-muted-foreground"
    >
      {/* The sentence says what is going on; the spinner only moves. */}
      <Spinner role="presentation" aria-hidden className="size-3" />
      {t(
        result?.meta?.generatedAt
          ? 'home.building.update'
          : 'home.building.first',
      )}
    </p>
  )
}
