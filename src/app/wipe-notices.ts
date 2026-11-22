/**
 * The one-time notices about the data folder (`take_wipe_notice`): a
 * "Delete all data" PathKeep undid, a record of one it set aside, files it
 * moved out of the way, a disk worth checking. The backend hands each notice
 * out once, so each one is a toast that stays until it is closed.
 *
 * Not responsible for: draining them. The session drains them once the
 * archive is open; the delete dialog drains the ones its own failed attempt
 * left, so a notice is never said twice.
 */
import { toast } from 'sonner'
import type { RecoveryNotice } from '@/lib/backend-client/archive-access'
import type { useI18n } from '@/lib/i18n'
import { reveal, revealLabelKey } from '@/features/settings/reveal'

type Translate = ReturnType<typeof useI18n>['t']

/** A notice the delete dialog already says in its own words. */
export const WIPE_ROLLED_BACK = 'wipe-rolled-back'

function words(t: Translate, notice: RecoveryNotice) {
  const path = notice.path ?? ''
  switch (notice.code) {
    case WIPE_ROLLED_BACK:
      return {
        title: t('archiveAccess.notices.wipeRolledBack'),
        body: t('archiveAccess.notices.wipeRolledBackBody'),
      }
    case 'wipe-record-discarded':
      return {
        title: t('archiveAccess.notices.wipeRecordDiscarded'),
        body: t('archiveAccess.notices.wipeRecordDiscardedBody'),
      }
    case 'wipe-files-displaced':
      return {
        title: t('archiveAccess.notices.wipeFilesDisplaced'),
        body: t('archiveAccess.notices.wipeFilesDisplacedBody', { path }),
      }
    case 'archive-check-disk':
      return {
        title: t('archiveAccess.notices.checkDisk'),
        body: t('archiveAccess.notices.checkDiskBody', { path }),
      }
    default:
      // A code a later backend adds is still said, with what it names.
      return {
        title: t('archiveAccess.notices.other'),
        body: [notice.code, notice.path].filter(Boolean).join(' · '),
      }
  }
}

export function announceWipeNotices(t: Translate, notices: RecoveryNotice[]) {
  for (const notice of notices) {
    const { title, body } = words(t, notice)
    const path = notice.path
    toast.warning(title, {
      id: `wipe-notice:${notice.code}:${path ?? ''}`,
      description: body,
      // Said once, often right after a launch: it stays until it is closed.
      duration: Infinity,
      closeButton: true,
      action: path
        ? {
            label: t(revealLabelKey()),
            onClick: () =>
              void reveal(path, t('settingsStorage.location.failed')),
          }
        : undefined,
    })
  }
}
