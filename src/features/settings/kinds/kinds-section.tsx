/**
 * Settings › Manage data types (`#/settings/kinds`, `data-spaces.md` §8):
 * every kind of data PathKeep keeps, each turned on, paused or off on its
 * own. None of the three deletes anything.
 *
 * Responsibilities: laying out one card per kind in the switcher's order,
 * from the backend's states and the sources found on this computer, and
 * working out which kind is the last one on.
 * Not in this build: deleting one kind's saved data, which arrives with
 * its own backend step.
 */
import { SettingsSection } from '@/components/app/setting-row'
import { CardError } from '@/components/app/section-card'
import { Skeleton } from '@/components/ui/skeleton'
import type { DetectedSources } from '@/lib/backend-client/data-kinds'
import { useI18n } from '@/lib/i18n'
import { useDataKinds, useDetectedSources } from '@/lib/queries/data-kinds'
import { spaces, type DataSpace } from '@/lib/spaces'
import { foundAgentTools, foundBrowsers } from '@/features/data-kinds/presence'
import { toolName } from '@/features/data-kinds/tools'
import { KindCard, type SourcesLine } from './kind-card'
import { useKindStateChange } from './use-kind-state-change'

function sourcesOf(
  space: DataSpace,
  detected: { data?: DetectedSources; isError: boolean },
  t: ReturnType<typeof useI18n>['t'],
): SourcesLine {
  if (detected.isError) return { state: 'failed' }
  if (!detected.data) return { state: 'loading' }
  const names =
    space.kind === 'agent-history'
      ? foundAgentTools(detected.data).map((tool) => t(toolName(tool)))
      : foundBrowsers(detected.data)
  return { state: 'found', names }
}

function CardSkeleton() {
  return (
    <div
      aria-hidden
      className="flex flex-col gap-3 rounded-xl border bg-card px-[18px] py-4 shadow-card"
    >
      <div className="flex items-start gap-3.5">
        <Skeleton className="size-9 rounded-lg" />
        <div className="flex flex-1 flex-col gap-1.5">
          <Skeleton className="h-4 w-32" />
          <Skeleton className="h-3.5 w-64" />
          <Skeleton className="h-3 w-40" />
        </div>
        <Skeleton className="h-8 w-44 rounded-md" />
      </div>
      <Skeleton className="ml-[50px] h-3 w-72" />
    </div>
  )
}

export function KindsSection() {
  const { t } = useI18n()
  const kinds = useDataKinds()
  const detected = useDetectedSources(['browsing', 'agent-history'])
  const { pending, change } = useKindStateChange()
  const list = kinds.data

  const onCount = list
    ? list.kinds.filter((row) => (pending[row.kind] ?? row.state) === 'on')
        .length
    : 0

  return (
    <SettingsSection title={t('dataKinds.manage.title')}>
      <p className="-mt-2 mb-1 text-[13px] leading-relaxed text-muted-foreground">
        {t('dataKinds.manage.intro')}
      </p>
      {kinds.isError && !list ? (
        <div className="rounded-xl border bg-card px-[18px] py-4 shadow-card">
          <CardError error={kinds.error} onRetry={() => void kinds.refetch()} />
        </div>
      ) : !list ? (
        spaces.map((space) => <CardSkeleton key={space.id} />)
      ) : (
        spaces.map((space) => {
          const row = list.kinds.find((item) => item.kind === space.kind)
          if (!row) return null
          const shownState = pending[space.kind] ?? row.state
          return (
            <KindCard
              key={space.id}
              space={space}
              row={row}
              shownState={shownState}
              lastOn={shownState === 'on' && onCount === 1}
              busy={pending[space.kind] !== undefined}
              sources={sourcesOf(space, detected, t)}
              onChange={(next) => void change(space, list, next)}
              onRetry={() => void kinds.refetch()}
            />
          )
        })
      )}
    </SettingsSection>
  )
}
