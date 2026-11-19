/**
 * Looking at folders from the backup page, the same walk the enable dialog
 * shows (it only counts files; nothing is opened or saved):
 * - "Add another folder…": pick a folder (the system picker in the app; in
 *   a browser, type its full path), say which tool's conversations it
 *   holds, look at it, then add it;
 * - a tool switched on that has no approved folder here yet: look at its
 *   folder, then add it;
 * - "Rescan this source": look at a tool's folders again and see what is
 *   in them now. It backs nothing up;
 * - a folder made again where an approved one was (same path, another
 *   folder: its `directoryToken` differs): look at it by its path, approve
 *   it again, and back up, since the backup it failed cannot be retried
 *   once its folder was replaced. A rescan that finds such a folder offers
 *   the same.
 *
 * Responsibilities: the walk for each case and the approval (every approved
 * folder plus the new or replaced readable ones, each once, with this
 * preview). A look that went stale or a set of sources changed meanwhile
 * asks for another look.
 */
import { FolderSearch } from 'lucide-react'
import { useId, useMemo, useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  AGENT_TOOLS,
  dataKindsErrorCode,
  type AgentPreviewRequest,
  type AgentToolId,
} from '@/lib/backend-client/data-kinds'
import { describeError } from '@/lib/errors'
import { useI18n, type MessageKey } from '@/lib/i18n'
import { hasTauriGuestApi } from '@/lib/runtime'
import { toolName } from '@/features/data-kinds/tools'
import { isFound } from '@/features/enable-agent/preview-model'
import { PreviewWalk } from '@/features/enable-agent/preview-step'
import { useAgentPreview } from '@/features/enable-agent/use-agent-preview'
import { sendSourceChange } from './use-source-changes'
import { useStartAgentBackup } from './use-start-backup'

export type SourceDialogMode =
  | { mode: 'folder' }
  | { mode: 'tool'; tool: AgentToolId }
  | { mode: 'rescan'; tool: AgentToolId; rootIds: string[] }
  | { mode: 'renew'; tool: AgentToolId; path: string; rootId: string }

/** An approved folder, and which folder at its path was approved. */
export interface ApprovedRoot {
  rootId: string
  directoryToken?: string
}

/**
 * A folder the look found that is approved by id but is another folder now.
 * Without a token on either side there is nothing to compare.
 */
function replaced(
  approved: readonly ApprovedRoot[],
  row: { rootId: string; directoryToken?: string },
) {
  const known = approved.find((item) => item.rootId === row.rootId)
  return (
    known !== undefined &&
    known.directoryToken !== undefined &&
    row.directoryToken !== undefined &&
    known.directoryToken !== row.directoryToken
  )
}

const addFailures: Partial<Record<string, MessageKey>> = {
  'scope-denied': 'agentBackup.add.reasons.scopeDenied',
  'source-selection-empty': 'agentBackup.add.reasons.empty',
  'preview-expired': 'agentBackup.add.reasons.expired',
  'enable-preview-required': 'agentBackup.add.reasons.expired',
  'root-changed': 'agentBackup.add.reasons.changed',
  'policy-changed': 'agentBackup.add.reasons.policy',
}

async function pickFolder(title: string) {
  const { open } = await import('@tauri-apps/plugin-dialog')
  const picked = await open({ title, multiple: false, directory: true })
  return typeof picked === 'string' ? picked : null
}

/** One look at the folders the request names, and approving what it found. */
function Look({
  active,
  request,
  tool,
  mode,
  renewing,
  approved,
  onClose,
}: {
  /** The dialog is open; closing it stops the walk. */
  active: boolean
  request: AgentPreviewRequest
  tool: AgentToolId
  mode: SourceDialogMode['mode']
  /** The approved folder looked at again by its path (renew mode). */
  renewing?: string
  approved: readonly ApprovedRoot[]
  onClose: () => void
}) {
  const { t } = useI18n()
  const preview = useAgentPreview(active, request)
  const backup = useStartAgentBackup()
  const [adding, setAdding] = useState(false)
  const [failure, setFailure] = useState<string | null>(null)
  const report =
    preview.state.phase === 'finished' ? preview.state.report : null
  const readable = report
    ? report.rows.filter((row) => isFound(row) && row.readable)
    : []
  const fresh = readable.filter(
    (row) => !approved.some((item) => item.rootId === row.rootId),
  )
  const renewed = readable.filter((row) => replaced(approved, row))
  // Adding offers what is new; a look at known folders offers only a
  // folder that was replaced. A look by path also offers a folder the path
  // now leads to under another identity, in place of the old one.
  const add = mode === 'folder' || mode === 'tool'
  const byPath = add || mode === 'renew'
  const approving = byPath ? [...fresh, ...renewed] : renewed
  const offer = byPath || renewed.length > 0
  const nothing = report !== null && approving.length === 0
  // The approved folder is still the one at its path: nothing to approve.
  const target = readable.find((row) => row.rootId === renewing)
  const unchanged =
    target !== undefined &&
    target.directoryToken !== undefined &&
    approved.some(
      (item) =>
        item.rootId === renewing &&
        item.directoryToken === target.directoryToken,
    )
  // Its path leads elsewhere now: the new folder takes its place.
  const dropped =
    mode === 'renew' && target === undefined && fresh.length > 0
      ? renewing
      : undefined

  const confirm = async () => {
    if (!report) return
    setAdding(true)
    setFailure(null)
    try {
      await sendSourceChange({
        previewId: report.previewId,
        selectedRootIds: [
          ...new Set([
            ...approved
              .map((item) => item.rootId)
              .filter((rootId) => rootId !== dropped),
            ...approving.map((row) => row.rootId),
          ]),
        ],
        ...(add ? { toolChanges: { [tool]: true } } : {}),
      })
      if (add) {
        toast(t('agentBackup.add.added'))
      } else {
        // The backup that failed on the old folder cannot be retried; a new
        // one reads the folder just approved.
        void backup.start()
      }
      onClose()
    } catch (error) {
      const known = addFailures[dataKindsErrorCode(error) ?? '']
      setFailure(
        known ? t(known) : describeError(error, 'update_agent_sources'),
      )
    } finally {
      setAdding(false)
    }
  }

  return (
    <>
      <PreviewWalk
        state={preview.state}
        onCancel={preview.cancel}
        onRerun={preview.rerun}
      />
      <p className="text-xs text-muted-foreground">
        {t('enableAgent.preview.nothingOpened')}
      </p>
      {(renewed.length > 0 || (mode === 'renew' && fresh.length > 0)) && (
        <p role="status" className="text-[13px]">
          {t('agentBackup.renew.replaced', { count: approving.length })}
        </p>
      )}
      {mode === 'renew' && nothing && (
        <p role="status" className="text-[13px] text-muted-foreground">
          {t(unchanged ? 'agentBackup.renew.same' : 'agentBackup.renew.none')}
        </p>
      )}
      {add && nothing && (
        <p role="status" className="text-[13px] text-muted-foreground">
          {t('agentBackup.add.nothingReadable')}
        </p>
      )}
      {failure && (
        <div
          role="alert"
          className="flex flex-col gap-1 rounded-lg border bg-card px-3.5 py-3 text-[13px]"
        >
          <span className="font-medium">{t('agentBackup.add.failed')}</span>
          <span className="text-xs [overflow-wrap:anywhere] text-muted-foreground">
            {failure}
          </span>
        </div>
      )}
      <DialogFooter className="gap-2 sm:gap-2">
        {offer ? (
          <>
            <Button variant="ghost" onClick={onClose}>
              {t('common.cancel')}
            </Button>
            <Button
              disabled={
                !report ||
                nothing ||
                adding ||
                (preview.state.phase === 'finished' && preview.state.expired)
              }
              onClick={() => void confirm()}
            >
              {t(add ? 'agentBackup.add.confirm' : 'agentBackup.renew.confirm')}
            </Button>
          </>
        ) : (
          <Button variant="outline" onClick={onClose}>
            {t('common.close')}
          </Button>
        )}
      </DialogFooter>
    </>
  )
}

/** The folder and tool fields of "Add another folder…". */
function FolderFields({
  onLook,
}: {
  onLook: (path: string, tool: AgentToolId) => void
}) {
  const { t } = useI18n()
  const pathId = useId()
  const toolId = useId()
  const [path, setPath] = useState('')
  const [tool, setTool] = useState<AgentToolId>('claude-code')
  const native = hasTauriGuestApi()
  return (
    <form
      className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_220px]"
      onSubmit={(event) => {
        event.preventDefault()
        if (path.trim()) onLook(path.trim(), tool)
      }}
    >
      <div className="flex flex-col gap-1.5">
        <Label htmlFor={pathId}>{t('agentBackup.add.folder')}</Label>
        <div className="flex gap-2">
          <Input
            id={pathId}
            value={path}
            spellCheck={false}
            className="font-mono text-xs"
            aria-describedby={native ? undefined : `${pathId}-hint`}
            onChange={(event) => setPath(event.target.value)}
          />
          {native && (
            <Button
              type="button"
              variant="outline"
              onClick={() =>
                void pickFolder(t('agentBackup.add.title')).then(
                  (picked) => picked && setPath(picked),
                )
              }
            >
              {t('agentBackup.add.choose')}
            </Button>
          )}
        </div>
        {!native && (
          <span id={`${pathId}-hint`} className="text-xs text-muted-foreground">
            {t('agentBackup.add.typePath')}
          </span>
        )}
      </div>
      <div className="flex flex-col gap-1.5">
        <Label htmlFor={toolId}>{t('agentBackup.add.tool')}</Label>
        <Select
          value={tool}
          onValueChange={(value) => setTool(value as AgentToolId)}
        >
          <SelectTrigger id={toolId} className="w-full">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {AGENT_TOOLS.map((id) => (
              <SelectItem key={id} value={id}>
                {t(toolName(id))}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      <Button
        type="submit"
        variant="outline"
        className="w-fit"
        disabled={!path.trim()}
      >
        <FolderSearch aria-hidden />
        {t('agentBackup.add.look')}
      </Button>
    </form>
  )
}

function DialogBody({
  open,
  dialog,
  approved,
  onClose,
}: {
  open: boolean
  dialog: SourceDialogMode
  approved: readonly ApprovedRoot[]
  onClose: () => void
}) {
  const { t } = useI18n()
  const [picked, setPicked] = useState<{
    path: string
    tool: AgentToolId
    look: number
  } | null>(null)
  const request = useMemo<AgentPreviewRequest | null>(() => {
    if (dialog.mode === 'rescan')
      return dialog.rootIds.length > 0
        ? { purpose: 'rescan', rootIds: dialog.rootIds }
        : { purpose: 'rescan', tools: [dialog.tool] }
    if (dialog.mode === 'tool')
      return { purpose: 'rescan', tools: [dialog.tool] }
    // By its path: a look by id refuses a folder that was replaced.
    if (dialog.mode === 'renew')
      return {
        purpose: 'rescan',
        pickedFolder: { tool: dialog.tool, path: dialog.path },
      }
    return picked
      ? {
          purpose: 'rescan',
          pickedFolder: { tool: picked.tool, path: picked.path },
        }
      : null
  }, [dialog, picked])

  const name = dialog.mode === 'folder' ? '' : t(toolName(dialog.tool))
  const title =
    dialog.mode === 'folder'
      ? t('agentBackup.add.title')
      : dialog.mode === 'tool'
        ? t('agentBackup.add.toolTitle', { name })
        : dialog.mode === 'renew'
          ? t('agentBackup.renew.title', { name })
          : t('agentBackup.rescan.title', { name })
  const body =
    dialog.mode === 'folder'
      ? t('agentBackup.add.body')
      : dialog.mode === 'tool'
        ? t('agentBackup.add.toolBody', { name })
        : dialog.mode === 'renew'
          ? t('agentBackup.renew.body', { path: dialog.path })
          : t('agentBackup.rescan.body')
  const tool =
    dialog.mode === 'folder' ? (picked?.tool ?? 'claude-code') : dialog.tool

  return (
    <>
      <DialogHeader>
        <DialogTitle>{title}</DialogTitle>
        <DialogDescription>{body}</DialogDescription>
      </DialogHeader>
      {dialog.mode === 'folder' && (
        <FolderFields
          onLook={(path, chosen) =>
            setPicked((now) => ({
              path,
              tool: chosen,
              look: (now?.look ?? 0) + 1,
            }))
          }
        />
      )}
      {request ? (
        <div
          key={picked?.look ?? 0}
          className="flex animate-rise flex-col gap-4"
        >
          <Look
            active={open}
            request={request}
            tool={tool}
            mode={dialog.mode}
            renewing={dialog.mode === 'renew' ? dialog.rootId : undefined}
            approved={approved}
            onClose={onClose}
          />
        </div>
      ) : (
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            {t('common.cancel')}
          </Button>
        </DialogFooter>
      )}
    </>
  )
}

export function SourcePreviewDialog({
  dialog,
  approved,
  onClose,
}: {
  dialog: SourceDialogMode | null
  approved: readonly ApprovedRoot[]
  onClose: () => void
}) {
  // The last one stays drawn while the dialog fades out; each opening is new.
  const [shown, setShown] = useState({ dialog, opening: 0 })
  if (dialog && dialog !== shown.dialog)
    setShown({ dialog, opening: shown.opening + 1 })
  return (
    <Dialog open={dialog !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="gap-4 sm:max-w-[760px]">
        {shown.dialog && (
          <DialogBody
            key={shown.opening}
            open={dialog !== null}
            dialog={shown.dialog}
            approved={approved}
            onClose={onClose}
          />
        )}
      </DialogContent>
    </Dialog>
  )
}
