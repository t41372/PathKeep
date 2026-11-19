/**
 * Settings › Manage data types (`#/settings/kinds`, `data-spaces.md` §8):
 * every kind of data PathKeep keeps, each turned on, paused or off on its
 * own. None of the three deletes anything.
 *
 * Responsibilities: laying out one card per kind in the switcher's order,
 * from the backend's states and the sources found on this computer, and
 * working out which kind is the last one on, or the one that has to stay on
 * while the other has no sources to back up. Sizes are asked for after the
 * cards are drawn (counting them reads the archive files); until they
 * come, or if they cannot be counted, the cards just leave them out.
 * Not in this build: deleting one kind's saved data, which arrives with
 * its own backend step.
 */
import { SettingsSection } from '@/components/app/setting-row'
import { CardError } from '@/components/app/section-card'
import { Skeleton } from '@/components/ui/skeleton'
import {
  needsSourceReview,
  type DetectedSources,
} from '@/lib/backend-client/data-kinds'
import { useI18n } from '@/lib/i18n'
import {
  useDataKinds,
  useDataKindSizes,
  useDetectedSources,
} from '@/lib/queries/data-kinds'
import { useNavigate } from 'react-router-dom'
import { spaceBackupPath, spaces, type DataSpace } from '@/lib/spaces'
import { foundAgentTools, foundBrowsers } from '@/features/data-kinds/presence'
import { useRetryStore } from '@/features/data-kinds/use-retry-store'
import { toolName } from '@/features/data-kinds/tools'
import { useOpenEnableAgent } from '@/features/enable-agent/links'
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
  const sizes = useDataKindSizes(kinds.isSuccess)
  const { pending, change } = useKindStateChange()
  const retry = useRetryStore()
  const openEnable = useOpenEnableAgent()
  const list = kinds.data

  const onCount = list
    ? list.kinds.filter((row) => (pending[row.kind] ?? row.state) === 'on')
        .length
    : 0
  // A kind on with nothing it can back up does not count: the other one
  // has to stay on until its sources are chosen.
  const waiting = list?.kinds.find((row) =>
    needsSourceReview(row, pending[row.kind] ?? row.state),
  )
  const navigate = useNavigate()

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
              heldBy={
                shownState === 'on' && waiting && waiting.kind !== space.kind
                  ? waiting
                  : undefined
              }
              busy={pending[space.kind] !== undefined}
              sources={sourcesOf(space, detected, t)}
              archiveBytes={
                sizes.data?.kinds.find((item) => item.kind === space.kind)
                  ?.archiveBytes
              }
              retrying={retry.busy}
              onChange={(next) => void change(space, list, next)}
              onRetry={() => void retry.retry()}
              onSetUp={openEnable}
              onReview={() => navigate(spaceBackupPath(space.id))}
            />
          )
        })
      )}
    </SettingsSection>
  )
}
