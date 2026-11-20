/**
 * One backup's details on the Agent backup page (`?run={runRef}`), also for
 * a run that is not in the list. A conversation backup shows how it ended,
 * when, how long, what started it, whether it read `.git`, what it counted,
 * each tool and folder, what it could not save, the format versions it met
 * that PathKeep cannot read yet, and the notes it left on files it saved
 * (a run with only notes succeeded, and says so), a page at a time; with
 * Try again and Dismiss from the run's own task, so an attempt from before
 * a restart or yesterday can still be retried. A run of the
 * browsing ledger (work on the whole archive, or an old numeric link) opens
 * the browsing page's own sheet, which already shows those, once its full
 * reference is known to still name it.
 */
import { useState, type ReactNode } from 'react'
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { RotateCcw } from 'lucide-react'
import type {
  AgentRunSummary,
  ArchiveRunDetail,
  KindTask,
} from '@/lib/backend-client/agent-backup'
import { dataKindsErrorCode } from '@/lib/backend-client/data-kinds'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n, type MessageKey } from '@/lib/i18n'
import {
  joinRunPages,
  useArchiveRun,
  wireNumber,
} from '@/lib/queries/agent-backup'
import { ToolBadge } from '@/features/data-kinds/tool-badge'
import { RunSheet } from '@/features/backup/run-sheet'
import { RunResultIcon } from './recent-runs'
import { useRunParam } from './run-param'
import { RunFormatNotices } from './format-notice-card'
import { NoteEntry, WarningEntry } from './run-entries'
import { useTaskControl } from './use-task-control'
import { availabilityText, runTook, taskName } from './words'

const resultWords: Record<string, MessageKey> = {
  running: 'agentBackup.run.result.running',
  success: 'agentBackup.run.result.success',
  partial: 'agentBackup.run.result.partial',
  interrupted: 'agentBackup.run.result.interrupted',
  failed: 'agentBackup.run.result.failed',
}

const triggerWords: Record<string, MessageKey> = {
  enable: 'agentBackup.run.trigger.enable',
  manual: 'agentBackup.run.trigger.manual',
  retry: 'agentBackup.run.trigger.retry',
  scheduled: 'agentBackup.run.trigger.schedule',
  'menu-bar': 'agentBackup.run.trigger.menuBar',
}

const COUNTS: (keyof AgentRunSummary & keyof typeof countWords)[] = [
  'newRoots',
  'updatedRoots',
  'newChildren',
  'filesCaptured',
  'filesUnchanged',
  'filesVisited',
  'filesAbsent',
  'unknownFormat',
  'unavailable',
  'unstable',
  'unclassified',
]

const countWords = {
  filesVisited: 'agentBackup.run.counts.filesVisited',
  filesCaptured: 'agentBackup.run.counts.filesCaptured',
  filesUnchanged: 'agentBackup.run.counts.filesUnchanged',
  filesAbsent: 'agentBackup.run.counts.filesAbsent',
  newRoots: 'agentBackup.run.counts.newRoots',
  updatedRoots: 'agentBackup.run.counts.updatedRoots',
  newChildren: 'agentBackup.run.counts.newChildren',
  unknownFormat: 'agentBackup.run.counts.unknownFormat',
  unavailable: 'agentBackup.run.counts.unavailable',
  unstable: 'agentBackup.run.counts.unstable',
  unclassified: 'agentBackup.run.counts.unclassified',
} as const satisfies Record<string, MessageKey>

/** The numeric id of a browsing-ledger run, `browsing:<origin>/<id>` or a bare id. */
function browsingRunId(runRef: string) {
  const bare = runRef.startsWith('browsing:')
    ? runRef.slice(runRef.lastIndexOf('/') + 1)
    : runRef
  return /^\d+$/.test(bare) ? Number(bare) : null
}

function Fact({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex justify-between gap-4">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="text-right">{children}</dd>
    </div>
  )
}

/**
 * Try again and Dismiss for the run itself, from its own task: also for an
 * attempt from before a restart or yesterday, which no list shows now.
 */
function RunActions({ task }: { task: KindTask }) {
  const { t } = useI18n()
  const control = useTaskControl()
  const name = taskName(t, task)
  const retry = task.actions.includes('retry')
  const dismiss = task.actions.includes('dismiss')
  if (!retry && !dismiss) return null
  return (
    <div className="flex flex-wrap gap-2">
      {retry && (
        <Button
          size="sm"
          variant="outline"
          aria-label={t('agentTasks.actions.retryLabel', { name })}
          disabled={control.busy !== null}
          onClick={() => void control.send(task, 'retry')}
        >
          <RotateCcw aria-hidden />
          {t('agentTasks.actions.retry')}
        </Button>
      )}
      {dismiss && (
        <Button
          size="sm"
          variant="ghost"
          aria-label={t('agentTasks.actions.dismissLabel', { name })}
          disabled={control.busy !== null}
          onClick={() => void control.send(task, 'dismiss')}
        >
          {t('agentTasks.actions.dismiss')}
        </Button>
      )}
    </div>
  )
}

function Detail({
  detail,
  more,
  loadingMore,
  onMore,
}: {
  detail: ArchiveRunDetail
  more: boolean
  loadingMore: boolean
  onMore: () => void
}) {
  const { t } = useI18n()
  const format = useFormat()
  const summary = 'newRoots' in detail.summary ? detail.summary : null
  const took = runTook(t, detail.durationMs)
  const counts = summary
    ? COUNTS.filter((key) => wireNumber(summary[key]) > 0)
    : []
  // Runs recorded before notes were counted listed everything as notes.
  const counted = detail.noteCount != null
  return (
    <div className="flex flex-col gap-5 px-4 pb-6 text-[13px]">
      {detail.task && <RunActions task={detail.task} />}
      <dl className="flex flex-col gap-2">
        <Fact label={t('agentBackup.run.started')}>
          {format.dayAndTime(detail.startedAt ?? 0)}
        </Fact>
        <Fact label={t('agentBackup.run.took')}>{took ?? '—'}</Fact>
        <Fact label={t('agentBackup.run.trigger.label')}>
          {triggerWords[detail.trigger]
            ? t(triggerWords[detail.trigger])
            : t('agentBackup.run.trigger.other', { trigger: detail.trigger })}
        </Fact>
        {detail.readGit !== undefined && (
          <Fact label={t('agentBackup.run.readGit')}>
            {t(detail.readGit ? 'agentBackup.run.yes' : 'agentBackup.run.no')}
          </Fact>
        )}
        {summary && (
          <Fact label={t('agentBackup.run.counts.sourceBytes')}>
            {format.bytes(wireNumber(summary.sourceBytes))}
          </Fact>
        )}
      </dl>
      {counts.length > 0 && (
        <dl className="grid grid-cols-2 gap-2">
          {counts.map((key) => (
            <div
              key={key}
              className="flex flex-col gap-0.5 rounded-lg bg-muted px-3 py-2"
            >
              <dt className="text-xs text-muted-foreground">
                {t(countWords[key])}
              </dt>
              <dd className="text-lg font-semibold tabular">
                {format.number(wireNumber(summary![key]))}
              </dd>
            </div>
          ))}
        </dl>
      )}
      {(detail.perTool?.length ?? 0) > 0 && (
        <section className="flex flex-col gap-2">
          <h3 className="font-medium">{t('agentBackup.run.tools')}</h3>
          <ul className="flex flex-col divide-y rounded-lg border">
            {detail.perTool!.map((tool) => (
              <li
                key={tool.tool}
                data-tool={tool.tool}
                className="flex flex-col gap-1 px-3 py-2.5"
              >
                <ToolBadge id={tool.tool} />
                <span className="text-xs text-muted-foreground">
                  {availabilityText(t, tool.availability)}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
      {(detail.roots?.length ?? 0) > 0 && (
        <section className="flex flex-col gap-2">
          <h3 className="font-medium">{t('agentBackup.run.folders')}</h3>
          <ul className="flex flex-col gap-1.5">
            {detail.roots!.map((root) => (
              <li key={root.rootId} className="flex flex-col gap-0.5">
                <span className="font-mono text-[11.5px] [overflow-wrap:anywhere]">
                  {root.path}
                </span>
                <span className="text-xs text-muted-foreground">
                  {availabilityText(t, root.availability)}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
      <section
        aria-label={t(
          counted ? 'agentBackup.run.warnings' : 'agentBackup.run.notes',
        )}
        className="flex flex-col gap-2"
      >
        <h3 className="font-medium">
          {t(counted ? 'agentBackup.run.warnings' : 'agentBackup.run.notes')}
        </h3>
        {(detail.warnings?.length ?? 0) === 0 ? (
          <p className="text-muted-foreground">
            {t(
              counted
                ? 'agentBackup.run.noWarnings'
                : 'agentBackup.run.noNotes',
            )}
          </p>
        ) : (
          <ul className="flex flex-col gap-1.5">
            {detail.warnings!.map((warning, index) => (
              <WarningEntry
                key={`${warning.code}-${index}`}
                warning={warning}
                counted={counted}
              />
            ))}
          </ul>
        )}
      </section>
      {(detail.formatNotices?.length ?? 0) > 0 && (
        <section
          aria-label={t('agentBackup.run.formats')}
          className="flex flex-col gap-2"
        >
          <h3 className="font-medium">{t('agentBackup.run.formats')}</h3>
          <RunFormatNotices
            task={detail.task}
            notices={detail.formatNotices!}
          />
        </section>
      )}
      {(detail.notes?.length ?? 0) > 0 && (
        <section
          aria-label={t('agentBackup.run.notes')}
          className="flex flex-col gap-2"
        >
          <h3 className="font-medium">
            {t('agentBackup.run.notes')}
            <span className="ml-1.5 font-normal text-muted-foreground tabular">
              {wireNumber(detail.noteCount) || detail.notes!.length}
            </span>
          </h3>
          <p className="text-muted-foreground">
            {t('agentBackup.run.notesBody')}
          </p>
          <ul className="flex flex-col gap-1.5">
            {detail.notes!.map((note, index) => (
              <NoteEntry
                key={`${note.code}-${note.path}-${index}`}
                note={note}
              />
            ))}
          </ul>
        </section>
      )}
      {more && (
        <Button
          variant="outline"
          size="sm"
          className="w-fit"
          disabled={loadingMore}
          onClick={onMore}
        >
          {t('agentBackup.run.moreDetail')}
        </Button>
      )}
    </div>
  )
}

function AgentRun({
  runRef,
  open,
  onClose,
}: {
  runRef: string
  open: boolean
  onClose: () => void
}) {
  const { t } = useI18n()
  const format = useFormat()
  const query = useArchiveRun(runRef)
  const detail = joinRunPages(query.data)
  const missing = dataKindsErrorCode(query.error) === 'record-unavailable'
  return (
    <Sheet open={open} onOpenChange={(next) => !next && onClose()}>
      <SheetContent className="w-[460px] overflow-y-auto sm:max-w-[460px]">
        <SheetHeader>
          <SheetTitle className="flex items-center gap-2">
            {detail && <RunResultIcon result={detail.result} />}
            {detail
              ? t('agentBackup.run.title', {
                  time: format.dayAndTime(detail.startedAt ?? 0),
                })
              : t('agentBackup.runs.title')}
          </SheetTitle>
          <SheetDescription>
            {detail
              ? [
                  t(
                    resultWords[detail.result] ??
                      'agentBackup.run.result.failed',
                  ),
                  wireNumber(detail.noteCount) > 0 &&
                    t('agentBackup.runs.notes', {
                      count: wireNumber(detail.noteCount),
                    }),
                ]
                  .filter(Boolean)
                  .join(t('agentTasks.progress.join'))
              : t('agentBackup.run.description')}
          </SheetDescription>
        </SheetHeader>
        {query.isPending ? (
          <div className="flex flex-col gap-3 px-4">
            <Skeleton className="h-16 w-full" />
            <Skeleton className="h-24 w-full" />
          </div>
        ) : !detail ? (
          <p
            role="alert"
            className="px-4 text-[13px] break-words text-destructive"
          >
            {missing
              ? t('agentBackup.run.notFound')
              : describeError(query.error, 'get_archive_run')}
          </p>
        ) : (
          <Detail
            detail={detail}
            more={query.hasNextPage}
            loadingMore={query.isFetchingNextPage}
            onMore={() => void query.fetchNextPage()}
          />
        )}
      </SheetContent>
    </Sheet>
  )
}

/**
 * A run of the browsing ledger by its full reference. The number alone
 * could name another run once the archive was restored or brought in (its
 * ledger starts again), so the reference is checked first; one that names
 * nothing any more says so instead of opening whatever has that number now.
 */
function BrowsingRun({
  runRef,
  open,
  onClose,
}: {
  runRef: string
  open: boolean
  onClose: () => void
}) {
  const { t } = useI18n()
  const qualified = runRef.startsWith('browsing:')
  const check = useArchiveRun(qualified ? runRef : null)
  const missing = dataKindsErrorCode(check.error) === 'record-unavailable'
  if (qualified && missing)
    return (
      <Sheet open={open} onOpenChange={(next) => !next && onClose()}>
        <SheetContent className="w-[460px] sm:max-w-[460px]">
          <SheetHeader>
            <SheetTitle>{t('agentBackup.runs.title')}</SheetTitle>
            <SheetDescription>{t('agentBackup.run.notFound')}</SheetDescription>
          </SheetHeader>
        </SheetContent>
      </Sheet>
    )
  // An answer that is not "missing" (also a refusal for another reason)
  // leaves it to the browsing sheet, which says what went wrong.
  const checked = !qualified || check.isSuccess || check.isError
  return (
    <RunSheet
      runId={open && checked ? browsingRunId(runRef) : null}
      onClose={onClose}
    />
  )
}

/** The sheet for `?run=`, whichever ledger the run is in. */
export function AgentRunSheet() {
  const { runRef, close } = useRunParam()
  // The last run stays drawn while the sheet slides away.
  const [last, setLast] = useState(runRef)
  if (runRef !== null && runRef !== last) setLast(runRef)
  if (last === null) return null
  if (last.startsWith('agent-history:'))
    return <AgentRun runRef={last} open={runRef !== null} onClose={close} />
  return <BrowsingRun runRef={last} open={runRef !== null} onClose={close} />
}
