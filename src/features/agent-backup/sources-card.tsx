/**
 * The backup page's "Sources" card: everything that decides what a
 * conversation backup reads, in one place (`data-spaces.md` §4): each
 * tool's switch, adding another folder, rescanning, and reading `.git`.
 *
 * Responsibilities: the rows (`source-rows.ts`), what a switch does (on for
 * a tool with no approved folder here looks at its folder first), and
 * holding the switches while a backup runs, since any change stops it.
 * On a computer that backs nothing up (conversations moved here from
 * another), the switches wait for "Set up backup…" in the header.
 */
import { FolderGit2, FolderPlus } from 'lucide-react'
import { useId, useMemo, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { Switch } from '@/components/ui/switch'
import type { AgentOverview } from '@/lib/backend-client/agent-backup'
import type { AgentToolId } from '@/lib/backend-client/data-kinds'
import { useI18n } from '@/lib/i18n'
import { useAgentSources } from '@/lib/queries/agent-backup'
import { kindStateOf } from '@/lib/spaces'
import { useRenewParam } from './review-param'
import {
  useAgentSettings,
  useDataKinds,
  useDetectedSources,
} from '@/lib/queries/data-kinds'
import { CardError } from '@/components/app/section-card'
import { foundAgentTools } from '@/features/data-kinds/presence'
import { toolName } from '@/features/data-kinds/tools'
import {
  SourcePreviewDialog,
  type SourceDialogMode,
} from './source-preview-dialog'
import { SourceRow } from './source-row'
import { toolRows } from './source-rows'
import { useSourceChanges } from './use-source-changes'

function RowsSkeleton() {
  return (
    <ul aria-hidden className="border-t">
      {[0, 1].map((index) => (
        <li
          key={index}
          className="flex items-center gap-3 border-t px-4 py-3 first:border-t-0"
        >
          <Skeleton className="h-5 w-9 rounded-full" />
          <Skeleton className="h-4 w-32" />
          <Skeleton className="ml-auto h-4 w-20" />
        </li>
      ))}
    </ul>
  )
}

export function SourcesCard({
  overview,
  running,
  viewOnly,
}: {
  overview: AgentOverview | undefined
  /** A backup of this kind is running: a change would stop it. */
  running: boolean
  viewOnly: boolean
}) {
  const { t } = useI18n()
  const noteId = useId()
  const lastId = useId()
  const gitTitle = useId()
  const gitBody = useId()
  const sources = useAgentSources()
  const settings = useAgentSettings()
  const detected = useDetectedSources(['agent-history'])
  const changes = useSourceChanges()
  const [dialog, setDialog] = useState<SourceDialogMode | null>(null)

  const items = useMemo(
    () => sources.data?.pages.flatMap((page) => page.items) ?? [],
    [sources.data],
  )
  const found = useMemo(
    () => new Set(detected.data ? foundAgentTools(detected.data) : []),
    [detected.data],
  )
  const rows = useMemo(
    () => toolRows(items, overview, found),
    [items, overview, found],
  )
  const approved = useMemo(
    () =>
      items.flatMap((item) =>
        item.rootId
          ? [{ rootId: item.rootId, directoryToken: item.directoryToken }]
          : [],
      ),
    [items],
  )
  const locked = running || viewOnly
  const readGit = settings.data?.policy.readGit ?? true
  // The only kind on cannot lose its last source: the backend refuses, so
  // that switch does not offer it (a folder not plugged in still counts).
  const kinds = useDataKinds()
  const soleOn =
    kindStateOf(kinds.data, 'agent-history') === 'on' &&
    kindStateOf(kinds.data, 'browsing') !== 'on'
  const backedUp = rows.filter((row) => row.enabled && row.roots.length > 0)
  const lastTool =
    soleOn && backedUp.length === 1 ? backedUp[0].tool : undefined

  // A folder to look at again, asked for by the address (a retry, a link).
  const renew = useRenewParam()
  const renewing = renew.rootId
    ? items.find((item) => item.rootId === renew.rootId)
    : undefined
  const renewTool = renewing?.tool
  const renewPath = renewing?.path
  const renewRoot = renewing?.rootId
  const asked = useMemo<SourceDialogMode | null>(
    () =>
      renewTool && renewPath && renewRoot
        ? { mode: 'renew', tool: renewTool, path: renewPath, rootId: renewRoot }
        : null,
    [renewTool, renewPath, renewRoot],
  )
  const shownDialog = dialog ?? asked
  const closeDialog = () => {
    setDialog(null)
    if (renew.rootId) renew.clear()
  }

  const onSwitch = async (tool: AgentToolId, on: boolean) => {
    const outcome = await changes.setTool(tool, on)
    // No folder of it is approved here yet: look at it first.
    if (outcome === 'needs-preview') setDialog({ mode: 'tool', tool })
  }

  return (
    <section
      aria-labelledby={`${noteId}-title`}
      className="overflow-hidden rounded-xl border bg-card shadow-card"
    >
      <header className="flex flex-wrap items-center gap-x-3 gap-y-1 px-5 py-3">
        <h2 id={`${noteId}-title`} className="text-sm font-semibold">
          {t('agentBackup.sources.title')}
        </h2>
        <span className="text-xs text-muted-foreground">
          {t('agentBackup.sources.subtitle')}
        </span>
        <span className="flex-1" />
        <Button
          variant="outline"
          size="sm"
          disabled={locked}
          aria-describedby={locked ? noteId : undefined}
          onClick={() => setDialog({ mode: 'folder' })}
        >
          <FolderPlus aria-hidden />
          {t('agentBackup.sources.add')}
        </Button>
      </header>
      {locked && (
        <p
          id={noteId}
          className="border-t bg-muted/30 px-5 py-2 text-xs text-muted-foreground"
        >
          {t(
            viewOnly
              ? 'agentBackup.sources.viewOnlyNote'
              : 'agentBackup.sources.busy',
          )}
        </p>
      )}
      {!locked && lastTool && (
        <p
          id={lastId}
          className="border-t bg-muted/30 px-5 py-2 text-xs text-muted-foreground"
        >
          {t('agentBackup.sources.lastSource', {
            name: t(toolName(lastTool)),
          })}
        </p>
      )}
      {sources.isError && !sources.data ? (
        <div className="border-t px-5 py-4">
          <CardError
            error={sources.error}
            onRetry={() => void sources.refetch()}
          />
        </div>
      ) : !sources.data || !overview ? (
        <RowsSkeleton />
      ) : rows.length === 0 ? (
        <div className="flex flex-col gap-0.5 border-t px-5 py-4 text-[13px]">
          <span className="font-medium">
            {t('agentBackup.sources.empty.title')}
          </span>
          <span className="text-muted-foreground">
            {t('agentBackup.sources.empty.body')}
          </span>
        </div>
      ) : (
        <ul className="border-t">
          {rows.map((row) => (
            <SourceRow
              key={row.tool}
              row={row}
              locked={locked}
              pending={changes.pending === row.tool}
              lastSourceNote={row.tool === lastTool ? lastId : undefined}
              onSwitch={(on) => void onSwitch(row.tool, on)}
              onRenew={(root) =>
                root.rootId &&
                root.path &&
                setDialog({
                  mode: 'renew',
                  tool: root.tool,
                  path: root.path,
                  rootId: root.rootId,
                })
              }
              onRescan={() => {
                // A look by id refuses a replaced folder: look at it by path.
                const changed = row.roots.find(
                  (root) =>
                    root.availability === 'root-changed' &&
                    root.rootId &&
                    root.path,
                )
                setDialog(
                  changed
                    ? {
                        mode: 'renew',
                        tool: changed.tool,
                        path: changed.path!,
                        rootId: changed.rootId!,
                      }
                    : {
                        mode: 'rescan',
                        tool: row.tool,
                        rootIds: row.roots.flatMap((root) =>
                          root.rootId ? [root.rootId] : [],
                        ),
                      },
                )
              }}
            />
          ))}
        </ul>
      )}
      {sources.hasNextPage && (
        <div className="border-t px-5 py-2">
          <Button
            variant="ghost"
            size="sm"
            disabled={sources.isFetchingNextPage}
            onClick={() => void sources.fetchNextPage()}
          >
            {t('agentBackup.sources.more')}
          </Button>
        </div>
      )}
      <div className="flex items-start gap-3 border-t bg-muted/30 px-5 py-3.5">
        <FolderGit2
          aria-hidden
          className="mt-0.5 size-4 shrink-0 text-muted-foreground"
        />
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span id={gitTitle} className="text-sm font-medium">
            {t('agentBackup.sources.git.title')}
          </span>
          <span
            id={gitBody}
            className="max-w-[80ch] text-[12.5px] leading-relaxed text-muted-foreground"
          >
            {t('agentBackup.sources.git.body')}
          </span>
        </span>
        <Switch
          aria-labelledby={gitTitle}
          aria-describedby={gitBody}
          checked={readGit}
          disabled={locked || !settings.data || changes.pending === 'git'}
          onCheckedChange={(on) => void changes.setReadGit(on)}
        />
      </div>
      <SourcePreviewDialog
        dialog={shownDialog}
        approved={approved}
        onClose={closeDialog}
      />
    </section>
  )
}
