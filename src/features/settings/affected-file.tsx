/**
 * The file a refusal names, on its own line: what it is to the archive, its
 * path, and the way to it in the file manager. Shown under the refusal
 * wherever one can name a file (repair, safety copy, move, encryption,
 * restore). A file that is gone is shown by its folder.
 *
 * `Refusal` is the plain refusal of a whole-archive operation: the dialog's
 * own words, or PathKeep's words for a file it couldn't write, then the file.
 *
 * Not responsible for: a dialog's own words for its refusals.
 */
import { FolderOpen } from 'lucide-react'
import type { ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { describeError } from '@/lib/errors'
import { useI18n, type MessageKey } from '@/lib/i18n'
import {
  commandErrorAffectedFile,
  isArchiveFileWriteFailed,
} from '@/lib/ipc/command-error'
import { revealFile, revealLabelKey } from './reveal'
import { snapshotFolderWords } from './snapshot-folder-words'

/** A role a later backend adds is said as a file of the archive. */
const roles: Record<string, MessageKey> = {
  browsing: 'maintenance.file.browsing',
  evidence: 'maintenance.file.evidence',
  readModel: 'maintenance.file.readModel',
  heldGeneration: 'maintenance.file.heldGeneration',
}

/** Nothing when the error names no file. */
export function AffectedFile({ error }: { error: unknown }) {
  const { t } = useI18n()
  const file = commandErrorAffectedFile(error)
  if (!file) return null
  return (
    <div
      data-affected-file={file.role}
      className="flex flex-col gap-1 text-[13px]"
    >
      <span className="text-muted-foreground">
        {t(roles[file.role] ?? 'maintenance.file.browsing')}
      </span>
      <span
        data-affected-path
        className="font-mono text-xs break-all text-foreground"
      >
        {file.path}
      </span>
      <Button
        type="button"
        size="sm"
        variant="outline"
        className="self-start"
        onClick={() =>
          void revealFile(file.path, t('settingsStorage.location.failed'))
        }
      >
        <FolderOpen aria-hidden />
        {t(revealLabelKey())}
      </Button>
    </div>
  )
}

/**
 * `children` are the dialog's words; a failed write is said the same way
 * everywhere, followed by the system's reason (a full disk and a refused
 * permission are fixed differently). A `raw-snapshots/` linked to another
 * disk is also said the same way everywhere.
 */
export function Refusal({
  error,
  id,
  children,
}: {
  error: unknown
  id?: string
  children: ReactNode
}) {
  const { t } = useI18n()
  return (
    <div className="flex flex-col gap-2">
      <p
        id={id}
        role="alert"
        className="text-[13px] [overflow-wrap:anywhere] text-destructive"
      >
        {isArchiveFileWriteFailed(error)
          ? `${t('maintenance.writeFailed')} ${describeError(error)}`
          : (snapshotFolderWords(t, error) ?? children)}
      </p>
      <AffectedFile error={error} />
    </div>
  )
}
