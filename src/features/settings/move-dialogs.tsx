/**
 * Moving PathKeep to another computer: pack everything into one `.pathkeep`
 * file (`export_app_data`), or open such a file here. Opening one replaces
 * this computer's archive, so it goes check (`preview_app_data_import`) →
 * confirm → replace (`apply_app_data_import`), then restarts the session.
 */
import { useMutation } from '@tanstack/react-query'
import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { useSession } from '@/app/session'
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
  IMPORT_SOURCE_KEY_INVALID_PREFIX,
  migrationClient,
  type ImportPreview,
} from '@/lib/backend-client/migration'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import { hasTauriGuestApi } from '@/lib/runtime'
import { Notice } from './notice'
import { BUNDLE_EXTENSION, PathField } from './path-field'
import { reveal, revealLabelKey } from './reveal'

type Mode = 'export' | 'import'

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

/** Downloads in the desktop app; the app's own exports folder otherwise. */
function useDefaultBundlePath() {
  const exportsDir = useSnapshot().directories.exportsDir
  const date = new Date().toISOString().slice(0, 10)
  const name = `pathkeep-${date}.${BUNDLE_EXTENSION}`
  const [path, setPath] = useState(`${exportsDir}/${name}`)
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

  const run = useMutation({
    mutationFn: () => migrationClient.exportAppData(path.trim()),
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
          if (path.trim() && !run.isPending) run.mutate()
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
        <p className="text-[13px] text-muted-foreground">
          {t('settingsStorage.move.stays')}
        </p>
        {run.isPending && (
          <div
            role="status"
            className="flex items-center gap-2 text-[13px] text-muted-foreground"
          >
            <Spinner />
            {t('settingsStorage.move.exporting')}
          </div>
        )}
        {run.error && (
          <p
            role="alert"
            className="text-[13px] [overflow-wrap:anywhere] text-destructive"
          >
            {t('settingsStorage.move.exportFailed', {
              message: describeError(run.error, 'export_app_data'),
            })}
          </p>
        )}
        <DialogFooter>
          <Button
            type="button"
            variant="ghost"
            disabled={run.isPending}
            onClick={onClose}
          >
            {t('common.cancel')}
          </Button>
          <Button type="submit" disabled={!path.trim() || run.isPending}>
            {t('settingsStorage.move.export')}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  )
}

function ImportForm({ onClose }: { onClose: () => void }) {
  const { t } = useI18n()
  const session = useSession()
  const [path, setPath] = useState('')
  const [password, setPassword] = useState('')
  const [preview, setPreview] = useState<ImportPreview | null>(null)
  const encrypted = preview?.manifest.archiveMode === 'encrypted'

  const check = useMutation({
    mutationFn: () => migrationClient.previewAppDataImport(path.trim()),
    onSuccess: setPreview,
  })
  const apply = useMutation({
    mutationFn: () =>
      migrationClient.applyAppDataImport(path.trim(), {
        confirmOverwrite: true,
        sourceArchiveKey: encrypted ? password : undefined,
      }),
    onSuccess: async () => {
      toast.success(t('settingsStorage.move.imported'))
      onClose()
      await session.restart()
    },
  })

  const busy = check.isPending || apply.isPending
  const wrongPassword =
    apply.error &&
    describeError(apply.error).includes(IMPORT_SOURCE_KEY_INVALID_PREFIX)

  return (
    <DialogContent className="sm:max-w-md" {...busyProps(apply.isPending)}>
      <form
        className="flex flex-col gap-4"
        onSubmit={(event) => {
          event.preventDefault()
          if (busy) return
          if (!preview) {
            if (path.trim()) check.mutate()
          } else if (!encrypted || password) {
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
              onChange={(event) => setPassword(event.target.value)}
            />
          </div>
        )}

        {preview?.willOverwriteExisting && (
          <Notice tone="warning">{t('settingsStorage.move.replaces')}</Notice>
        )}

        {apply.isPending && (
          <div
            role="status"
            className="flex items-center gap-2 text-[13px] text-muted-foreground"
          >
            <Spinner />
            {t('settingsStorage.move.importing')}
          </div>
        )}

        {(check.error ?? apply.error) && (
          <p
            role="alert"
            className="text-[13px] [overflow-wrap:anywhere] text-destructive"
          >
            {wrongPassword
              ? t('settingsStorage.move.wrongPassword')
              : t(
                  check.error
                    ? 'settingsStorage.move.checkFailed'
                    : 'settingsStorage.move.importFailed',
                  {
                    message: describeError(check.error ?? apply.error),
                  },
                )}
          </p>
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
              disabled={busy || (encrypted && !password)}
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
    [t('settingsStorage.move.summary.version'), manifest.appVersion],
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
    </div>
  )
}
