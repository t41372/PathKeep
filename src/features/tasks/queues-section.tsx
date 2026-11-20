/**
 * "Queues and data" under the four groups of the background tasks page:
 * everything Settings showed under Background work that is state, not a
 * setting (`data-spaces.md` §5.2), with the same rows, words, buttons and
 * confirmations: the Insights queue (with Rebuild), the search-index and
 * assistant queue, page summaries, link previews, then the data the work
 * builds (Insights data with Clear…, extra page details). All of it is
 * browsing history's, so it carries that space's tile and is left out
 * while browsing history is off.
 *
 * Not responsible for: the rows themselves (`features/settings/background/`).
 */
import { SettingsGroup } from '@/components/app/setting-row'
import { useI18n } from '@/lib/i18n'
import { useDataKinds } from '@/lib/queries/data-kinds'
import { spaceById } from '@/lib/spaces'
import { SpaceTile } from '@/app/shell/space-switcher'
import {
  InsightsDataRow,
  PageDetailsRow,
} from '@/features/settings/background/derived-data-rows'
import {
  LinkPreviewsRow,
  PageSummariesRow,
} from '@/features/settings/background/online-work-rows'
import {
  AiQueueRow,
  InsightsQueueRow,
} from '@/features/settings/background/queue-rows'

export function QueuesSection() {
  const { t } = useI18n()
  const kinds = useDataKinds()
  const state = kinds.data?.kinds.find((row) => row.kind === 'browsing')?.state
  if (state === 'off') return null
  const browsing = spaceById('browsing')!
  return (
    <section
      aria-labelledby="tasks-queues"
      className="mt-2 flex flex-col gap-3"
    >
      <div className="flex flex-col gap-1 px-1">
        <h2
          id="tasks-queues"
          className="flex items-center gap-2 text-[15px] font-semibold"
        >
          <SpaceTile space={browsing} size="sm" />
          {t('tasks.queues.title')}
          <span className="sr-only">{t(browsing.name)}</span>
        </h2>
        <p className="text-[13px] text-muted-foreground">
          {t('tasks.queues.note')}
        </p>
      </div>
      <InsightsQueueRow />
      <AiQueueRow />
      <PageSummariesRow />
      <LinkPreviewsRow />
      <SettingsGroup
        title={t('settingsBackground.data.group')}
        note={t('settingsBackground.data.note')}
      />
      <InsightsDataRow />
      <PageDetailsRow />
    </section>
  )
}
