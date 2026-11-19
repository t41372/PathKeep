/**
 * One kind of data in Manage data types: its icon, name, why it is kept,
 * the sources found on this computer, its on / paused / off control, and a
 * line saying what that state means and how much the archive holds.
 *
 * The last kind that is on cannot be paused or turned off: those choices
 * are disabled and the card says why. A kind whose storage will not open
 * says which file is damaged or unreadable, when that was found, that
 * browsing is unaffected, and offers to look again.
 */
import { CircleAlert } from 'lucide-react'
import { useId } from 'react'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import type { DataKindRow, KindState } from '@/lib/backend-client/data-kinds'
import { useI18n, type MessageKey, type Translator } from '@/lib/i18n'
import type { DataSpace } from '@/lib/spaces'
import { damageWords } from './kind-damage'

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
      : 'dataKinds.manage.count.sourceFiles',
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

function Damage({
  space,
  row,
  onRetry,
}: {
  space: DataSpace
  row: DataKindRow
  onRetry: () => void
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
        onClick={onRetry}
      >
        {t('dataKinds.damage.retry')}
      </Button>
    </div>
  )
}

export function KindCard({
  space,
  row,
  shownState,
  lastOn,
  busy,
  sources,
  onChange,
  onRetry,
}: {
  space: DataSpace
  row: DataKindRow
  /** The state to show: the one asked for while the change is on its way. */
  shownState: KindState
  /** The only kind on: it can be neither paused nor turned off. */
  lastOn: boolean
  busy: boolean
  sources: SourcesLine
  onChange: (next: KindState) => void
  /** Asks the backend again whether the kind's storage opens. */
  onRetry: () => void
}) {
  const { t } = useI18n()
  const reasonId = useId()
  const Icon = space.icon
  const name = t(space.name)
  const count = holding(t, row)
  const hint = t(stateHint[shownState])
  const notReady = row.availability === 'not-initialized'

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
          aria-describedby={lastOn ? reasonId : undefined}
          className="transition-opacity data-[disabled]:opacity-80"
        >
          {STATES.map((state) => (
            <ToggleGroupItem
              key={state}
              value={state}
              className="px-3"
              disabled={lastOn && state !== 'on'}
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
        <p className="text-xs text-muted-foreground">
          {notReady
            ? t('dataKinds.manage.notInitialized')
            : count === null
              ? t('dataKinds.manage.hintOnly', { hint })
              : t('dataKinds.manage.holds', { hint, count })}
        </p>
        {row.availability === 'unavailable' && (
          <Damage space={space} row={row} onRetry={onRetry} />
        )}
      </div>
    </section>
  )
}
