/**
 * Step 4: where the archive will be created and roughly how big it gets.
 *
 * The location comes from the app's data folder and can't be changed from
 * the UI, so the step offers to show the folder instead of a dead "Change"
 * button. With only browsing history chosen, three numbers: the browsers'
 * history files, the archive's size after the first backup, and how many
 * browsers. With more kinds, the same three numbers per kind and a total.
 *
 * Browsing's estimate uses file sizes only; visit counts aren't known until
 * the first backup reads the history. Agent conversations' comes from the
 * backend's walk of the ticked tools (`estimatedArchiveBytes`, measured
 * ratios per kind of file); when not every tool PathKeep can read is
 * ticked, the ticked ones are walked on their own, since the estimate is
 * one number for what was walked.
 */
import { FolderOpen } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { useAgentPreview } from '@/features/enable-agent/use-agent-preview'
import { wire } from '@/features/enable-agent/preview-model'
import { supportClient } from '@/lib/backend-client/support'
import type {
  AgentToolId,
  DataKindId,
  PreviewReport,
} from '@/lib/backend-client/data-kinds'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import { estimateOnboardingStorage } from '@/lib/onboarding-estimates'
import { useSnapshot } from '@/lib/queries/app'
import { isMacOsHost } from '@/lib/runtime'
import { spaceOfKind } from '@/lib/spaces'
import { walkFailureText } from './agent-words'
import { InlineError } from './inline-error'
import type { AgentSources } from './use-agent-sources'

interface Row {
  kind: DataKindId
  sourceBytes: number
  afterBytes: number
  sources: number
}

export function StorageStep({
  kinds,
  selected,
  agent,
  agentTools,
}: {
  kinds: DataKindId[]
  selected: string[]
  agent: AgentSources
  /** The ticked Agent tools PathKeep can read. */
  agentTools: AgentToolId[]
}) {
  const { t } = useI18n()
  const format = useFormat()
  const snapshot = useSnapshot()
  const [revealError, setRevealError] = useState<string | null>(null)
  const root = snapshot.directories.appRoot
  const browsing = estimateOnboardingStorage(snapshot.browserProfiles, selected)
  const agentReport = useAgentEstimate(kinds, agent, agentTools)
  const onlyBrowsing = kinds.length === 1 && kinds[0] === 'browsing'

  async function reveal() {
    setRevealError(null)
    try {
      await supportClient.openPathInFileManager(root)
    } catch (error) {
      setRevealError(describeError(error, 'open_path_in_file_manager'))
    }
  }

  const tiles = [
    {
      label: t('onboarding.storage.history'),
      value: format.bytes(browsing.sourceBytes),
    },
    {
      label: t('onboarding.storage.estimate'),
      value: `≈ ${format.bytes(browsing.totalBytes)}`,
    },
    {
      label: t('onboarding.storage.browsers'),
      value: format.number(browsing.profileCount),
    },
  ]

  return (
    <>
      <section className="flex flex-col gap-3.5 rounded-xl border bg-card p-[18px] shadow-card">
        <h2 className="text-[13px] font-medium">
          {t('onboarding.storage.location')}
        </h2>
        <div className="flex gap-2">
          <div
            className="flex h-9 min-w-0 flex-1 items-center truncate rounded-lg border bg-popover px-3 font-mono text-[13px]"
            title={root}
          >
            <span className="truncate">{root}</span>
          </div>
          <Button
            variant="outline"
            className="h-9"
            onClick={() => void reveal()}
          >
            <FolderOpen />
            {t(
              isMacOsHost()
                ? 'onboarding.storage.revealMac'
                : 'onboarding.storage.revealOther',
            )}
          </Button>
        </div>
        {revealError && (
          <InlineError
            title={t('onboarding.storage.revealFailed')}
            detail={revealError}
          />
        )}
      </section>
      {onlyBrowsing ? (
        <dl className="grid grid-cols-3 gap-2.5">
          {tiles.map((tile) => (
            <div
              key={tile.label}
              className="flex flex-col gap-1 rounded-[10px] border bg-card p-3.5 shadow-card"
            >
              <dt className="text-xs text-muted-foreground">{tile.label}</dt>
              <dd className="text-xl font-semibold tabular">{tile.value}</dd>
            </div>
          ))}
        </dl>
      ) : (
        <KindTable
          kinds={kinds}
          browsing={{
            kind: 'browsing',
            sourceBytes: browsing.sourceBytes,
            afterBytes: browsing.totalBytes,
            sources: browsing.profileCount,
          }}
          agent={agentReport}
          agentTools={agentTools.length}
        />
      )}
      <p className="text-[13px] leading-normal text-muted-foreground">
        {t(
          onlyBrowsing
            ? 'onboarding.storage.note'
            : 'onboarding.storage.noteKinds',
        )}
      </p>
    </>
  )
}

type AgentEstimate =
  | { phase: 'counting' }
  | { phase: 'ready'; report: PreviewReport }
  | { phase: 'failed'; message: string; retry: () => void }

/**
 * The walk whose estimate fits the ticked tools: the sources step's when
 * every tool PathKeep can read is ticked, else one of the ticked tools alone.
 */
function useAgentEstimate(
  kinds: DataKindId[],
  agent: AgentSources,
  agentTools: AgentToolId[],
): AgentEstimate {
  const { t } = useI18n()
  const wanted = kinds.includes('agent-history') && agentTools.length > 0
  const all =
    agent.readable !== null &&
    agent.readable.length === agentTools.length &&
    agent.readable.every((tool) => agentTools.includes(tool))
  const own = useAgentPreview(wanted && !all, {
    purpose: 'onboarding',
    tools: agentTools,
  })
  const preview = all ? agent.state : own.state
  const rerun = all ? agent.rerun : own.rerun
  switch (preview.phase) {
    case 'finished':
      return { phase: 'ready', report: preview.report }
    case 'walking':
      return { phase: 'counting' }
    case 'failed':
      return {
        phase: 'failed',
        message: walkFailureText(t, preview.code),
        retry: rerun,
      }
    case 'cancelled':
      return {
        phase: 'failed',
        message: walkFailureText(t, 'cancelled'),
        retry: rerun,
      }
  }
}

function KindTable({
  kinds,
  browsing,
  agent,
  agentTools,
}: {
  kinds: DataKindId[]
  browsing: Row
  agent: AgentEstimate
  agentTools: number
}) {
  const { t } = useI18n()
  const format = useFormat()
  const agentRow: Row | null =
    agent.phase === 'ready'
      ? {
          kind: 'agent-history',
          sourceBytes: wire(agent.report.totals.sourceBytes),
          afterBytes: wire(agent.report.estimatedArchiveBytes),
          sources: agentTools,
        }
      : null
  const rows = kinds.map((kind) => (kind === 'browsing' ? browsing : agentRow))
  const known = rows.every((row) => row !== null)
  const total = (field: 'sourceBytes' | 'afterBytes' | 'sources') =>
    rows.reduce((sum, row) => sum + (row?.[field] ?? 0), 0)

  return (
    <section className="flex flex-col gap-3 rounded-xl border bg-card p-[18px] shadow-card">
      <table className="w-full text-[13px]" aria-busy={!known}>
        <caption className="sr-only">
          {t('onboarding.storage.tableLabel')}
        </caption>
        <thead>
          <tr className="text-left text-xs text-muted-foreground">
            <th scope="col" className="pb-2 font-normal">
              {t('onboarding.storage.kind')}
            </th>
            <th scope="col" className="pb-2 text-right font-normal">
              {t('onboarding.storage.now')}
            </th>
            <th scope="col" className="pb-2 text-right font-normal">
              {t('onboarding.storage.after')}
            </th>
            <th scope="col" className="pb-2 text-right font-normal">
              {t('onboarding.storage.sources')}
            </th>
          </tr>
        </thead>
        <tbody>
          {kinds.map((kind, index) => {
            const space = spaceOfKind(kind)!
            const row = rows[index]
            return (
              <tr key={kind} className="border-t">
                <th scope="row" className="py-2 text-left font-normal">
                  <span className="flex items-center gap-2">
                    <space.icon
                      className="size-3.5 text-muted-foreground"
                      aria-hidden
                    />
                    {t(space.name)}
                  </span>
                </th>
                {row ? (
                  <>
                    <td className="py-2 text-right font-mono tabular">
                      {format.bytes(row.sourceBytes)}
                    </td>
                    <td className="py-2 text-right font-mono tabular">
                      {t('onboarding.storage.approx', {
                        size: format.bytes(row.afterBytes),
                      })}
                    </td>
                    <td className="py-2 text-right font-mono tabular">
                      {format.number(row.sources)}
                    </td>
                  </>
                ) : (
                  <SkeletonCells />
                )}
              </tr>
            )
          })}
          {kinds.length > 1 && (
            <tr className="border-t font-medium">
              <th scope="row" className="pt-2.5 text-left font-medium">
                {t('onboarding.storage.total')}
              </th>
              {known ? (
                <>
                  <td className="pt-2.5 text-right font-mono tabular">
                    {format.bytes(total('sourceBytes'))}
                  </td>
                  <td className="pt-2.5 text-right font-mono tabular">
                    {t('onboarding.storage.approx', {
                      size: format.bytes(total('afterBytes')),
                    })}
                  </td>
                  <td className="pt-2.5 text-right font-mono tabular">
                    {format.number(total('sources'))}
                  </td>
                </>
              ) : (
                <SkeletonCells />
              )}
            </tr>
          )}
        </tbody>
      </table>
      {agent.phase === 'counting' && (
        <span className="sr-only" aria-live="polite">
          {t('onboarding.storage.counting')}
        </span>
      )}
      {agent.phase === 'failed' && (
        <InlineError
          title={t('onboarding.storage.agentFailed')}
          detail={agent.message}
          actions={
            <Button size="sm" variant="outline" onClick={agent.retry}>
              {t('onboarding.storage.retry')}
            </Button>
          }
        />
      )}
    </section>
  )
}

function SkeletonCells() {
  return (
    <>
      {[16, 20, 6].map((width) => (
        <td key={width} className="py-2" aria-hidden>
          <Skeleton
            className="ml-auto h-4"
            style={{ width: `${width * 4}px` }}
          />
        </td>
      ))}
    </>
  )
}
