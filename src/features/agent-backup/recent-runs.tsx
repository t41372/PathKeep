/**
 * "Recent backups" on the Agent backup page, folded by default: folded, one
 * line with the newest backup's time and how many of those listed did not
 * save everything; unfolded, a row per backup with how it ended, when, how
 * long it took, what it did for Agent conversations, what it could not save,
 * and its notes (conditions of files it did save: a backup with only notes
 * succeeded, and reads that way). Work done to
 * the whole archive (health checks, clean-ups) is listed too, as on every
 * space's backup page. A row opens its details (`?run=`).
 *
 * Responsibilities: the list, a page at a time through the backend's
 * cursor (an hourly schedule makes thousands a year); never the whole ledger.
 */
import { ChevronDown, CircleCheck, Loader2, TriangleAlert } from 'lucide-react'
import { useState } from 'react'
import { CardError } from '@/components/app/section-card'
import { Button } from '@/components/ui/button'
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from '@/components/ui/collapsible'
import { Skeleton } from '@/components/ui/skeleton'
import type { ArchiveRun } from '@/lib/backend-client/agent-backup'
import { cn } from '@/lib/cn'
import {
  useFormat,
  useI18n,
  type MessageKey,
  type Translator,
} from '@/lib/i18n'
import { useAgentRuns, wireNumber } from '@/lib/queries/agent-backup'
import { useRunParam } from './run-param'
import { runSummaryText, runTook } from './words'

function hasWarnings(run: ArchiveRun) {
  return wireNumber(run.warningCount) > 0 || run.result === 'partial'
}

export function RunResultIcon({ result }: { result: string }) {
  if (result === 'success')
    return <CircleCheck aria-hidden className="size-4 shrink-0 text-green" />
  if (result === 'running')
    return (
      <Loader2
        aria-hidden
        className="size-4 shrink-0 animate-spin text-muted-foreground"
      />
    )
  return (
    <TriangleAlert
      aria-hidden
      className={cn(
        'size-4 shrink-0',
        result === 'failed' ? 'text-red' : 'text-brand',
      )}
    />
  )
}

const archiveWork: Record<string, MessageKey> = {
  doctor: 'agentBackup.runs.archive.doctor',
  retention_prune: 'agentBackup.runs.archive.retention_prune',
}

/** What the run did, in this space's terms. */
function runLine(t: Translator, run: ArchiveRun) {
  if (run.scope !== 'agent-history')
    return t(archiveWork[run.operation] ?? 'agentBackup.runs.archive.other')
  if (run.result === 'running') return t('agentBackup.runs.result.running')
  if (run.result === 'failed') return t('agentBackup.runs.result.failed')
  const did =
    'newRoots' in run.summary
      ? runSummaryText(t, run.summary, t('dataKinds.listSeparator'))
      : ''
  return run.result === 'interrupted'
    ? `${t('agentBackup.runs.result.interrupted')} · ${did}`
    : did
}

function RunRow({ run }: { run: ArchiveRun }) {
  const { t } = useI18n()
  const format = useFormat()
  const { open } = useRunParam()
  const time = format.dayAndTime(run.startedAt ?? 0)
  const took = runTook(t, run.durationMs)
  const warnings = wireNumber(run.warningCount)
  const notes = wireNumber(run.noteCount)
  const fresh = wireNumber(run.newNoteCount)
  return (
    <li className="border-t" data-run-ref={run.runRef}>
      <button
        type="button"
        aria-label={t('agentBackup.runs.open', { time })}
        onClick={() => open(run.runRef)}
        className="flex w-full items-start gap-3 px-5 py-3 text-left text-[13px] transition-colors hover:bg-accent/60"
      >
        <span className="mt-0.5">
          <RunResultIcon result={run.result} />
        </span>
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="flex flex-wrap items-baseline gap-x-2">
            <span className="font-medium tabular">{time}</span>
            {took && (
              <span className="text-xs text-muted-foreground">
                {t('agentBackup.runs.took', { duration: took })}
              </span>
            )}
          </span>
          <span className="text-xs text-muted-foreground">
            {runLine(t, run)}
          </span>
          {warnings > 0 && (
            <span className="text-xs text-brand">
              {t('agentBackup.runs.warnings', { count: warnings })}
            </span>
          )}
          {notes > 0 && (
            // Notes are about files that are saved: quiet, never a warning.
            <span className="text-xs text-muted-foreground tabular">
              {t('agentBackup.runs.notes', { count: notes })}
              {fresh > 0 &&
                `${t('agentTasks.progress.join')}${t('agentBackup.runs.newNotes', { count: fresh })}`}
            </span>
          )}
        </span>
      </button>
    </li>
  )
}

export function RecentRuns() {
  const { t } = useI18n()
  const format = useFormat()
  const [open, setOpen] = useState(false)
  const runs = useAgentRuns()
  const items = runs.data?.pages.flatMap((page) => page.items) ?? []
  const newest = items[0]
  const warned = items.filter(hasWarnings).length

  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      className="overflow-hidden rounded-xl border bg-card shadow-card"
    >
      <CollapsibleTrigger className="group flex w-full items-center gap-3 px-5 py-3.5 text-left">
        <span className="text-sm font-semibold">
          {t('agentBackup.runs.title')}
        </span>
        <span className="truncate text-[13px] text-muted-foreground tabular">
          {runs.isPending ? (
            <Skeleton aria-hidden className="h-3.5 w-40" />
          ) : !newest ? (
            t('agentBackup.runs.none')
          ) : warned > 0 ? (
            t('agentBackup.runs.warned', {
              time: format.dayAndTime(newest.startedAt ?? 0),
              count: warned,
            })
          ) : (
            t('agentBackup.runs.allFine', {
              time: format.dayAndTime(newest.startedAt ?? 0),
            })
          )}
        </span>
        <ChevronDown
          aria-hidden
          className="ml-auto size-4 shrink-0 text-muted-foreground transition-transform duration-200 group-data-[state=open]:rotate-180"
        />
      </CollapsibleTrigger>
      <CollapsibleContent className="overflow-hidden data-[state=closed]:animate-collapsible-up data-[state=open]:animate-collapsible-down">
        {runs.isError && !runs.data ? (
          <div className="border-t px-5 py-4">
            <CardError error={runs.error} onRetry={() => void runs.refetch()} />
          </div>
        ) : items.length === 0 ? (
          <p className="border-t px-5 py-4 text-[13px] text-muted-foreground">
            {t('agentBackup.runs.none')}
          </p>
        ) : (
          <ul className="max-h-[520px] overflow-y-auto">
            {items.map((run) => (
              <RunRow key={run.runRef} run={run} />
            ))}
          </ul>
        )}
        {runs.hasNextPage && (
          <div className="border-t px-5 py-2">
            <Button
              variant="ghost"
              size="sm"
              disabled={runs.isFetchingNextPage}
              onClick={() => void runs.fetchNextPage()}
            >
              {t('agentBackup.runs.more')}
            </Button>
          </div>
        )}
      </CollapsibleContent>
    </Collapsible>
  )
}
