/**
 * The words for a refusal about the data folder itself rather than the
 * operation: `raw-snapshots/` is a link to another disk, so the archive a
 * change replaces can't be moved into the safety copies. Said the same way
 * in every place that can meet it (import, restore, encryption, undoing an
 * import, the recovery screen).
 */
import type { useI18n } from '@/lib/i18n'
import { commandErrorCode } from '@/lib/ipc/command-error'

const SNAPSHOT_FOLDER_CROSS_DEVICE = 'snapshot-folder-cross-device'

/** `null` for any other error. */
export function snapshotFolderWords(
  t: ReturnType<typeof useI18n>['t'],
  error: unknown,
) {
  return commandErrorCode(error) === SNAPSHOT_FOLDER_CROSS_DEVICE
    ? t('maintenance.snapshotCrossDevice')
    : null
}
