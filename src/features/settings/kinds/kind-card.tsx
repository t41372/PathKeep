/**
 * One kind of data in Manage data types: its icon, name, why it is kept,
 * the sources found on this computer, its on / paused / off control, and a
 * line saying what that state means and how much the archive holds.
 *
 * The last kind that is on cannot be paused or turned off: those choices
 * are disabled and the card says why. A kind whose storage will not open
 * says which file is damaged or unreadable, when that was found, that
 * browsing is unaffected, and offers to try again (`store-damage.tsx`).
 * Agent conversations that open, except some that can't be read until their
 * lists are rebuilt, say that calmly and offer the repair
 * (`parked-notice.tsx`).
 * Agent conversations held from another computer, with nothing here
 * backed up, say so and offer to set backup up. A kind that is on with
 * nothing it can back up (browsing with no browser chosen; conversations
 * whose approved folders did not come with an imported archive) says so
 * and opens its backup page, where its sources are chosen; while it does,
 * the other kind
 * cannot be paused or turned off either, and says why. The size in the
 * archive joins the line once it is counted, after the cards are drawn.
 */
import { useId } from 'react'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import {
  isViewOnly,
  needsSourceReview,
  type DataKindRow,
  type KindState,
} from '@/lib/backend-client/data-kinds'
import {
  useFormat,
  useI18n,
  type MessageKey,
  type Translator,
} from '@/lib/i18n'
import type { DataSpace } from '@/lib/spaces'
import { ParkedNotice } from '@/features/agent-repair/parked-notice'
import { StoreDamage } from '@/features/data-kinds/store-damage'

const STATES: readonly KindState[] = ['on', 'paused', 'off']

const stateLabel: Record<KindState, MessageKey> = {
  on: 'dataKinds.manage.states.on',
  paused: 'dataKinds.manage.states.paused',
  off: 'dataKinds.manage.states.off',
}

const stateHint: Record<KindState, MessageKey> = {
  on: 'dataKinds.manage.hints.on',
  paused: 'dataKinds.manage.hints.paused',
  off: 'dataKinds.manage.hints.off',
}

/** What the archive holds, in the kind's own unit. */
function holding(t: Translator, row: DataKindRow) {
  if (row.archiveCount === undefined) return null
  const count = Number(row.archiveCount) || 0
  return t(
    row.countUnit === 'visits'
      ? 'dataKinds.manage.count.visits'
      : 'dataKinds.manage.count.conversations',
    { count },
  )
}

/** The sources line: names found, none, still looking, or could not check. */
export type SourcesLine =
  | { state: 'loading' }
  | { state: 'failed' }
  | { state: 'found'; names: string[] }

function Sources({ sources }: { sources: SourcesLine }) {
  const { t } = useI18n()
  if (sources.state === 'loading') {
    return (
      <span className="flex items-center text-xs text-muted-foreground">
        <span className="sr-only">{t('dataKinds.manage.sourcesLoading')}</span>
        <Skeleton aria-hidden className="my-0.5 h-3 w-40" />
      </span>
    )
  }
  return (
    <span className="text-xs text-muted-foreground">
      {sources.state === 'failed'
        ? t('dataKinds.manage.sourcesFailed')
        : sources.names.length === 0
          ? t('dataKinds.manage.sourcesNone')
          : t('dataKinds.manage.sources', {
              list: sources.names.join(t('dataKinds.listSeparator')),
            })}
    </span>
  )
}

export function KindCard({
  space,
  row,
  shownState,
  lastOn,
  heldBy,
  busy,
  sources,
  archiveBytes,
  retrying,
  onChange,
  onRetry,
  onSetUp,
  onReview,
}: {
  space: DataSpace
  row: DataKindRow
  /** The state to show: the one asked for while the change is on its way. */
  shownState: KindState
  /** The only kind on: it can be neither paused nor turned off. */
  lastOn: boolean
  /**
   * The other kind is on but has nothing to back up, so this one has to
   * stay on until that one's sources are chosen.
   */
  heldBy: DataKindRow | undefined
  busy: boolean
  sources: SourcesLine
  /** Its size in the archive, once counted (`list_data_kinds` with sizes). */
  archiveBytes: string | undefined
  retrying: boolean
  onChange: (next: KindState) => void
  /** Asks the backend to open the kind's storage again. */
  onRetry: () => void
  /** Opens the preview, for conversations kept from another computer. */
  onSetUp: () => void
  /** Opens where this kind's sources are chosen again. */
  onReview: () => void
}) {
  const { t } = useI18n()
  const format = useFormat()
  const reasonId = useId()
  const Icon = space.icon
  const name = t(space.name)
  const count = holding(t, row)
  const hint = t(stateHint[shownState])
  const notReady = row.availability === 'not-initialized'
  const held = !lastOn && heldBy !== undefined
  const reviewing = needsSourceReview(row)

  return (
    <section
      aria-label={name}
      data-kind={row.kind}
      className="flex flex-col gap-3 rounded-xl border bg-card px-[18px] py-4 shadow-card"
    >
      <div className="flex flex-wrap items-start gap-3.5">
        <span className="flex size-9 shrink-0 items-center justify-center rounded-lg bg-muted">
          <Icon aria-hidden className="size-[18px]" strokeWidth={1.75} />
        </span>
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <h3 className="font-medium">{name}</h3>
          <span className="text-[13px] leading-[1.45] text-muted-foreground">
            {t(space.why)}
          </span>
          <Sources sources={sources} />
        </div>
        <ToggleGroup
          type="single"
          variant="outline"
          size="sm"
          value={shownState}
          disabled={busy || notReady}
          onValueChange={(value) => {
            if (value) onChange(value as KindState)
          }}
          aria-label={t('dataKinds.manage.stateLabel', { name })}
          aria-describedby={lastOn || held ? reasonId : undefined}
          className="transition-opacity data-[disabled]:opacity-80"
        >
          {STATES.map((state) => (
            <ToggleGroupItem
              key={state}
              value={state}
              className="px-3"
              disabled={(lastOn || held) && state !== 'on'}
            >
              {t(stateLabel[state])}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      </div>
      <div className="flex flex-col gap-2 border-t pt-3 pl-[50px]">
        {lastOn && (
          <p id={reasonId} className="text-xs text-foreground">
            {t('dataKinds.manage.lastOn', { name })}
          </p>
        )}
        {held && (
          <p id={reasonId} className="text-xs text-foreground">
            {t('dataKinds.manage.heldOn', {
              name,
              other: t(
                heldBy.kind === 'browsing'
                  ? 'spaces.browsing.name'
                  : 'spaces.agent.name',
              ),
            })}
          </p>
        )}
        <p className="text-xs text-muted-foreground">
          {notReady
            ? t('dataKinds.manage.notInitialized')
            : count === null
              ? t('dataKinds.manage.hintOnly', { hint })
              : archiveBytes === undefined
                ? t('dataKinds.manage.holds', { hint, count })
                : t('dataKinds.manage.holdsSize', {
                    hint,
                    count,
                    size: format.bytes(Number(archiveBytes) || 0),
                  })}
        </p>
        {isViewOnly(row) && (
          <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5">
            <p className="max-w-[60ch] flex-1 text-xs text-foreground">
              {t('dataKinds.manage.viewOnly')}
            </p>
            <Button size="sm" variant="outline" onClick={onSetUp}>
              {t('dataKinds.manage.setUp')}
            </Button>
          </div>
        )}
        {reviewing && (
          <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5">
            <p className="max-w-[60ch] flex-1 text-xs text-foreground">
              {t(
                row.kind === 'browsing'
                  ? 'dataKinds.manage.review.browsing'
                  : 'dataKinds.manage.review.agent',
              )}
            </p>
            <Button size="sm" variant="outline" onClick={onReview}>
              {t(
                row.kind === 'browsing'
                  ? 'dataKinds.manage.review.chooseBrowsers'
                  : 'dataKinds.manage.review.chooseSources',
              )}
            </Button>
          </div>
        )}
        <ParkedNotice row={row} />
        {row.availability === 'unavailable' && (
          <StoreDamage
            space={space}
            row={row}
            busy={retrying}
            onRetry={onRetry}
          />
        )}
      </div>
    </section>
  )
}
