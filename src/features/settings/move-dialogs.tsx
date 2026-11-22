/**
 * Moving PathKeep to another computer: pack everything into one `.pathkeep`
 * file (`export_app_data`), or open such a file here. The file is always
 * encrypted with a password chosen at export, which becomes the archive's
 * password on the computer that opens it. Opening one replaces this
 * computer's browsing history, so it goes check (`preview_app_data_import`)
 * → confirm → replace (`apply_app_data_import`), then restarts the session.
 * Agent conversations are not replaced: this computer's stay and the file's
 * are added (user decision 2026-11-15), and the check says so, along with
 * how to take back what a wrong file added. When the conversations here
 * can't be read without the archive's own password, the import asks for it
 * (`agent-history-key-required`) in a field of its own, apart from the
 * file's password. A file PathKeep 0.5 or earlier made says so instead of
 * the version it records (they all record 0.1.0).
 * While saved Agent conversations can't be read, both are refused before
 * anything changes; the dialog says so and leads to the repair, or, when
 * the system only refuses access for now, to "Try again".
 */
import { useMutation } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { useSession } from '@/app/session'
import { MaintenanceProgress } from '@/components/app/maintenance-progress'
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
import { Spinner } from '@/components/ui/spinner'
import {
  migrationClient,
  type ImportPreview,
} from '@/lib/backend-client/migration'
import { describeError } from '@/lib/errors'
import { commandErrorCode } from '@/lib/ipc/command-error'
import { useFormat, useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import { hasTauriGuestApi } from '@/lib/runtime'
import {
  needsConversationAccess,
  needsConversationRepair,
} from '@/features/agent-repair/repair-errors'
import { AccessOffer, RepairOffer } from '@/features/agent-repair/repair-offer'
import { Refusal } from './affected-file'
import { Notice } from './notice'
import { BUNDLE_EXTENSION, PathField } from './path-field'
import { announceReplaced } from './replaced-notice'
import { reveal, revealLabelKey } from './reveal'

type Mode = 'export' | 'import'

/**
 * A file only a newer PathKeep reads: its format, found at the check, or the archive it holds,
 * found at the import. This computer's archive cannot be the one these name: it was let in when
 * the app started, and while it is open no other PathKeep can change it.
 */
const NEWER_FILE_CODES: (string | null)[] = [
  'move-file-too-new',
  'archive-reader-too-old',
  'archive-writer-too-old',
  'archive-config-format-too-new',
]

export function MoveDialog({
  mode,
  onClose,
}: {
  mode: Mode | null
  onClose: () => void
}) {
  return (
    <Dialog open={mode !== null} onOpenChange={(open) => !open && onClose()}>
      {mode === 'export' && <ExportForm onClose={onClose} />}
      {mode === 'import' && <ImportForm onClose={onClose} />}
    </Dialog>
  )
}

/**
 * Downloads in the desktop app; otherwise the one folder inside the data
 * folder a move file may be written to (`exports/pathkeep/format-3/`).
 */
function useDefaultBundlePath() {
  const exportsDir = useSnapshot().directories.exportsDir
  const date = new Date().toISOString().slice(0, 10)
  const name = `pathkeep-${date}.${BUNDLE_EXTENSION}`
  const [path, setPath] = useState(`${exportsDir}/pathkeep/format-3/${name}`)
  useEffect(() => {
    if (!hasTauriGuestApi()) return
    void import('@tauri-apps/api/path')
      .then(({ downloadDir, join }) =>
        downloadDir().then((dir) => join(dir, name)),
      )
      .then(setPath)
      .catch(() => undefined)
  }, [name])
  return [path, setPath] as const
}

function busyProps(busy: boolean) {
  return {
    showCloseButton: !busy,
    onInteractOutside: (event: Event) => busy && event.preventDefault(),
    onEscapeKeyDown: (event: KeyboardEvent) => busy && event.preventDefault(),
  }
}

function ExportForm({ onClose }: { onClose: () => void }) {
  const { t } = useI18n()
  const format = useFormat()
  const [path, setPath] = useDefaultBundlePath()
  const [password, setPassword] = useState('')
  const [confirm, setConfirm] = useState('')
  const differ = confirm !== '' && confirm !== password
  const ready = path.trim() !== '' && password !== '' && confirm === password
  const [stopped, setStopped] = useState(false)

  const run = useMutation({
    mutationFn: () => {
      setStopped(false)
      return migrationClient.exportAppData(path.trim(), password)
    },
    onSuccess: (bundle) => {
      toast.success(
        t('settingsStorage.move.exported', {
          size: format.bytes(bundle.bytesWritten),
        }),
        {
          description: bundle.bundlePath,
          action: {
            label: t(revealLabelKey()),
            onClick: () =>
              void reveal(
                bundle.bundlePath,
                t('settingsStorage.location.failed'),
              ),
          },
        },
      )
      onClose()
    },
  })

  return (
    <DialogContent className="sm:max-w-md" {...busyProps(run.isPending)}>
      <form
        className="flex flex-col gap-4"
        onSubmit={(event) => {
          event.preventDefault()
          if (ready && !run.isPending) run.mutate()
        }}
      >
        <DialogHeader>
          <DialogTitle>{t('settingsStorage.move.exportTitle')}</DialogTitle>
          <DialogDescription>
            {t('settingsStorage.move.exportIntro')}
          </DialogDescription>
        </DialogHeader>
        <PathField
          id="move-export-path"
          label={t('settingsStorage.move.saveTo')}
          mode="save"
          value={path}
          onChange={setPath}
          disabled={run.isPending}
        />
        <div className="flex flex-col gap-1.5">
          <Label htmlFor="move-export-password">
            {t('settingsStorage.move.exportPassword')}
          </Label>
          <Input
            id="move-export-password"
            type="password"
            autoComplete="new-password"
            value={password}
            disabled={run.isPending}
            onChange={(event) => setPassword(event.target.value)}
          />
        </div>
        <div className="flex flex-col gap-1.5">
          <Label htmlFor="move-export-password-confirm">
            {t('settingsStorage.move.exportPasswordConfirm')}
          </Label>
          <Input
            id="move-export-password-confirm"
            type="password"
            autoComplete="new-password"
            value={confirm}
            disabled={run.isPending}
            aria-invalid={differ}
            aria-describedby="move-export-password-hint"
            onChange={(event) => setConfirm(event.target.value)}
          />
          <p
            id="move-export-password-hint"
            className={
              differ
                ? 'text-[13px] text-destructive'
                : 'text-[13px] text-muted-foreground'
            }
          >
            {differ
              ? t('settingsStorage.move.passwordsDiffer')
              : t('settingsStorage.move.exportPasswordHint')}
          </p>
        </div>
        <p className="text-[13px] text-muted-foreground">
          {t('settingsStorage.move.stays')}
        </p>
        {run.isPending && (
          <MaintenanceProgress onStop={() => setStopped(true)} />
        )}
        {run.error &&
          (stopped ? (
            <p role="status" className="text-[13px] text-muted-foreground">
              {t('maintenance.stopped')}
            </p>
          ) : needsConversationRepair(run.error) ? (
            <RepairOffer
              operation="export"
              error={run.error}
              onLeave={onClose}
            />
          ) : needsConversationAccess(run.error) ? (
            <AccessOffer operation="export" error={run.error} />
          ) : (
            <Refusal error={run.error}>
              {t('settingsStorage.move.exportFailed', {
                message: describeError(run.error, 'export_app_data'),
              })}
            </Refusal>
          ))}
        <DialogFooter>
          <Button
            type="button"
            variant="ghost"
            disabled={run.isPending}
            onClick={onClose}
          >
            {t('common.cancel')}
          </Button>
          <Button type="submit" disabled={!ready || run.isPending}>
            {t('settingsStorage.move.export')}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  )
}

/** Backend code: the conversations here need the archive's own password to be kept. */
const NEEDS_CURRENT = 'agent-history-key-required'

function ImportForm({ onClose }: { onClose: () => void }) {
  const { t } = useI18n()
  const session = useSession()
  const [path, setPath] = useState('')
  const [password, setPassword] = useState('')
  const [currentPassword, setCurrentPassword] = useState('')
  const [askCurrent, setAskCurrent] = useState(false)
  const [preview, setPreview] = useState<ImportPreview | null>(null)
  const encrypted = preview?.manifest.archiveMode === 'encrypted'

  const check = useMutation({
    mutationFn: () => migrationClient.previewAppDataImport(path.trim()),
    onSuccess: setPreview,
  })
  const [stopped, setStopped] = useState(false)
  const apply = useMutation({
    mutationFn: () => {
      setStopped(false)
      return migrationClient.applyAppDataImport(
        path.trim(),
        {
          confirmOverwrite: true,
          sourceArchiveKey: encrypted ? password : undefined,
        },
        askCurrent ? currentPassword : null,
      )
    },
    onError: (error) => {
      if (commandErrorCode(error) === NEEDS_CURRENT) setAskCurrent(true)
    },
    onSuccess: async (result) => {
      announceReplaced(
        t,
        t('settingsStorage.move.imported'),
        result.previousCopyUnchecked,
      )
      onClose()
      await session.restart()
    },
  })

  const busy = check.isPending || apply.isPending
  const wrongPassword =
    apply.error !== null &&
    commandErrorCode(apply.error) === 'archive-password-wrong'
  const applyFailure =
    stopped || (apply.error && commandErrorCode(apply.error) === NEEDS_CURRENT)
      ? null
      : apply.error
  const failure = check.error ?? applyFailure
  const repairFirst = failure !== null && needsConversationRepair(failure)
  const accessFirst = failure !== null && needsConversationAccess(failure)
  const newerFile =
    failure !== null && NEWER_FILE_CODES.includes(commandErrorCode(failure))

  return (
    <DialogContent className="sm:max-w-md" {...busyProps(apply.isPending)}>
      <form
        className="flex flex-col gap-4"
        onSubmit={(event) => {
          event.preventDefault()
          if (busy) return
          if (!preview) {
            if (path.trim()) check.mutate()
          } else if (
            (!encrypted || password) &&
            (!askCurrent || currentPassword)
          ) {
            apply.mutate()
          }
        }}
      >
        <DialogHeader>
          <DialogTitle>{t('settingsStorage.move.importTitle')}</DialogTitle>
          <DialogDescription>
            {t('settingsStorage.move.importIntro')}
          </DialogDescription>
        </DialogHeader>

        {!preview ? (
          <PathField
            id="move-import-path"
            label={t('settingsStorage.move.file')}
            mode="open"
            value={path}
            onChange={setPath}
            disabled={busy}
          />
        ) : (
          <BundleSummary preview={preview} />
        )}

        {preview && encrypted && (
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="move-import-password">
              {t('settingsStorage.move.sourcePassword')}
            </Label>
            <Input
              id="move-import-password"
              type="password"
              autoComplete="off"
              value={password}
              disabled={apply.isPending}
              aria-invalid={Boolean(wrongPassword)}
              aria-describedby={
                preview.manifest.formatVersion === 1
                  ? 'move-import-password-hint'
                  : undefined
              }
              onChange={(event) => setPassword(event.target.value)}
            />
            {preview.manifest.formatVersion === 1 && (
              <p
                id="move-import-password-hint"
                className="text-xs text-muted-foreground"
              >
                {t('settingsStorage.move.oldPasswordHint')}
              </p>
            )}
          </div>
        )}

        {preview && askCurrent && (
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="move-import-current-password">
              {t('settingsStorage.move.currentPassword')}
            </Label>
            <Input
              id="move-import-current-password"
              type="password"
              autoComplete="current-password"
              value={currentPassword}
              disabled={apply.isPending}
              aria-describedby="move-import-current-password-hint"
              onChange={(event) => setCurrentPassword(event.target.value)}
            />
            <p
              id="move-import-current-password-hint"
              className="text-xs text-muted-foreground"
            >
              {t('settingsStorage.move.currentPasswordHint')}
            </p>
          </div>
        )}

        {preview?.willOverwriteExisting && (
          <Notice tone="warning">{t('settingsStorage.move.replaces')}</Notice>
        )}

        {apply.isPending && (
          <MaintenanceProgress onStop={() => setStopped(true)} />
        )}

        {apply.error && stopped && (
          <p role="status" className="text-[13px] text-muted-foreground">
            {t('maintenance.stopped')}
          </p>
        )}
        {repairFirst && (
          <RepairOffer operation="import" error={failure} onLeave={onClose} />
        )}
        {accessFirst && <AccessOffer operation="import" error={failure} />}
        {failure && !repairFirst && !accessFirst && (
          <Refusal error={failure}>
            {wrongPassword
              ? t(
                  askCurrent
                    ? 'settingsStorage.move.wrongEitherPassword'
                    : 'settingsStorage.move.wrongPassword',
                )
              : newerFile
                ? t('settingsStorage.move.newerFile')
                : t(
                    check.error
                      ? 'settingsStorage.move.checkFailed'
                      : 'settingsStorage.move.importFailed',
                    {
                      message: describeError(failure),
                    },
                  )}
          </Refusal>
        )}

        <DialogFooter>
          <Button
            type="button"
            variant="ghost"
            disabled={busy}
            onClick={preview ? () => setPreview(null) : onClose}
          >
            {preview ? t('common.back') : t('common.cancel')}
          </Button>
          {preview ? (
            <Button
              type="submit"
              variant={
                preview.willOverwriteExisting ? 'destructive' : 'default'
              }
              disabled={
                busy ||
                (encrypted && !password) ||
                (askCurrent && !currentPassword)
              }
            >
              {t(
                preview.willOverwriteExisting
                  ? 'settingsStorage.move.replace'
                  : 'settingsStorage.move.import',
              )}
            </Button>
          ) : (
            <Button type="submit" disabled={busy || !path.trim()}>
              {check.isPending && <Spinner />}
              {t('settingsStorage.move.check')}
            </Button>
          )}
        </DialogFooter>
      </form>
    </DialogContent>
  )
}

function BundleSummary({ preview }: { preview: ImportPreview }) {
  const { t } = useI18n()
  const format = useFormat()
  const manifest = preview.manifest
  const agent = agentOutcome(preview)
  const rows: [string, string][] = [
    [
      t('settingsStorage.move.summary.made'),
      [
        format.date(manifest.exportedAt, {
          dateStyle: 'medium',
          timeStyle: 'short',
        }),
        manifest.exporterHostname,
      ]
        .filter(Boolean)
        .join(' · '),
    ],
    [
      t('settingsStorage.move.summary.version'),
      preview.versionKnown
        ? manifest.appVersion
        : t('settingsStorage.move.summary.versionRange'),
    ],
    [
      t('settingsStorage.move.summary.encrypted'),
      manifest.archiveMode === 'encrypted'
        ? t('settingsStorage.move.summary.yes')
        : t('settingsStorage.move.summary.no'),
    ],
    [
      t('settingsStorage.move.summary.size'),
      format.bytes(preview.bytesToExtract),
    ],
    [
      t('settingsStorage.move.summary.agent'),
      t(`settingsStorage.move.summary.agentState.${agent}`),
    ],
  ]
  return (
    <div className="flex flex-col gap-2 rounded-lg bg-muted p-3 text-[13px]">
      {rows.map(([label, value]) => (
        <div key={label} className="flex justify-between gap-4">
          <span className="text-muted-foreground">{label}</span>
          <span className="text-right">{value}</span>
        </div>
      ))}
      {!preview.schemaUpToDate && (
        <p className="text-muted-foreground">
          {t('settingsStorage.move.summary.upgrade')}
        </p>
      )}
      {manifest.formatVersion === 1 && (
        <p className="text-muted-foreground">
          {t('settingsStorage.move.summary.noChats')}
        </p>
      )}
      {manifest.formatVersion <= 2 && (
        <p className="text-muted-foreground">
          {t('settingsStorage.move.summary.copies')}
        </p>
      )}
      <p className="text-muted-foreground">
        {t(`settingsStorage.move.summary.agentNote.${agent}`)}
      </p>
      {preview.agentConversations.keepsLocal && (
        <p data-keeps-local className="text-muted-foreground">
          {t(
            preview.agentConversations.carried
              ? 'settingsStorage.move.summary.keepsLocal'
              : 'settingsStorage.move.summary.keepsLocalNone',
          )}
        </p>
      )}
      {preview.agentConversations.carried &&
        preview.agentConversations.restoreKeepsAdditions && (
          <p className="text-muted-foreground">
            {t('settingsStorage.move.summary.undoNote')}
          </p>
        )}
    </div>
  )
}

/** What the import does to Agent conversations here, as the summary words it. */
function agentOutcome(preview: ImportPreview) {
  const facts = preview.agentConversations
  if (!facts.carried || facts.stateIfPresent === null) return 'none'
  return facts.stateIfPresent
}
