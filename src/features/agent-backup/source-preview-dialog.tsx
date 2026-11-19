/**
 * Looking at folders from the backup page, the same walk the enable dialog
 * shows (it only counts files; nothing is opened or saved):
 * - "Add another folder…": pick a folder (the system picker in the app; in
 *   a browser, type its full path), say which tool's conversations it
 *   holds, look at it, then add it;
 * - a tool switched on that has no approved folder here yet: look at its
 *   folder, then add it;
 * - "Rescan this source": look at a tool's folders again and see what is
 *   in them now. It backs nothing up.
 *
 * Responsibilities: the walk for each case and the add (every approved
 * folder plus the new readable ones, with this preview). A look that went
 * stale or a set of sources changed meanwhile asks for another look.
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

export type SourceDialogMode =
  | { mode: 'folder' }
  | { mode: 'tool'; tool: AgentToolId }
  | { mode: 'rescan'; tool: AgentToolId; rootIds: string[] }

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

/** One look at the folders the request names, and adding what it found. */
function Look({
  active,
  request,
  tool,
  add,
  approvedRootIds,
  onClose,
}: {
  /** The dialog is open; closing it stops the walk. */
  active: boolean
  request: AgentPreviewRequest
  tool: AgentToolId
  add: boolean
  approvedRootIds: readonly string[]
  onClose: () => void
}) {
  const { t } = useI18n()
  const preview = useAgentPreview(active, request)
  const [adding, setAdding] = useState(false)
  const [failure, setFailure] = useState<string | null>(null)
  const report =
    preview.state.phase === 'finished' ? preview.state.report : null
  const fresh = report
    ? report.rows.filter(
        (row) =>
          isFound(row) && row.readable && !approvedRootIds.includes(row.rootId),
      )
    : []
  const nothing = report !== null && fresh.length === 0

  const confirm = async () => {
    if (!report) return
    setAdding(true)
    setFailure(null)
    try {
      await sendSourceChange({
        previewId: report.previewId,
        selectedRootIds: [
          ...approvedRootIds,
          ...fresh.map((row) => row.rootId),
        ],
        toolChanges: { [tool]: true },
      })
      toast(t('agentBackup.add.added'))
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
        {add ? (
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
              {t('agentBackup.add.confirm')}
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
  approvedRootIds,
  onClose,
}: {
  open: boolean
  dialog: SourceDialogMode
  approvedRootIds: readonly string[]
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
        : t('agentBackup.rescan.title', { name })
  const body =
    dialog.mode === 'folder'
      ? t('agentBackup.add.body')
      : dialog.mode === 'tool'
        ? t('agentBackup.add.toolBody', { name })
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
            add={dialog.mode !== 'rescan'}
            approvedRootIds={approvedRootIds}
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
  approvedRootIds,
  onClose,
}: {
  dialog: SourceDialogMode | null
  approvedRootIds: readonly string[]
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
            approvedRootIds={approvedRootIds}
            onClose={onClose}
          />
        )}
      </DialogContent>
    </Dialog>
  )
}
