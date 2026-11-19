/**
 * Where a retry goes when the backend answers that the folders must be
 * looked at again (`previewRequired`), so the person lands on the one
 * thing to do instead of a dead end.
 *
 * - Conversations held from elsewhere, or on with no approved folder here
 *   (an import left none): the enable dialog, over the current screen, to
 *   choose folders.
 * - A folder made again where an approved one was: the look at that folder
 *   on the backup page (`?renew=`), which approves it and backs up. The
 *   old attempt itself cannot be retried once its folder was replaced.
 * - Otherwise (a tool switched off since, another `.git` choice): a notice
 *   that a new backup reads what is chosen now, with the backup page.
 *
 * Not responsible for: the dialogs themselves.
 */
import { toast } from 'sonner'
import { agentBackupClient } from '@/lib/backend-client/agent-backup'
import {
  agentReviewKind,
  isViewOnly,
  type DataKindList,
} from '@/lib/backend-client/data-kinds'
import { useI18n } from '@/lib/i18n'
import { queryClient, queryKeys } from '@/lib/query'
import { spaceBackupPath } from '@/lib/spaces'
import { useOpenEnableAgent } from '@/features/enable-agent/links'
import { renewPath } from './review-param'

async function sourceRows() {
  try {
    return (await agentBackupClient.sources(100)).items
  } catch {
    return undefined
  }
}

export function useOpenReview() {
  const { t } = useI18n()
  const openEnable = useOpenEnableAgent()
  return async () => {
    const kinds = queryClient.getQueryData<DataKindList>(queryKeys.dataKinds)
    const row = kinds?.kinds.find((item) => item.kind === 'agent-history')
    const rows = await sourceRows()
    const approvedHere = rows?.some((source) => source.rootId !== null)
    if (
      isViewOnly(row) ||
      agentReviewKind(row, approvedHere) === 'choose-folders'
    ) {
      openEnable()
      return
    }
    const changed = rows?.find(
      (source) =>
        source.enabled &&
        source.rootId !== null &&
        source.path !== null &&
        source.availability === 'root-changed',
    )
    if (changed?.rootId) {
      window.location.hash = renewPath(changed.rootId)
      return
    }
    toast(t('agentTasks.control.previewRequired'), {
      duration: 10_000,
      action: {
        label: t('agentTasks.control.openBackupPage'),
        onClick: () => {
          window.location.hash = spaceBackupPath('agent')
        },
      },
    })
  }
}
