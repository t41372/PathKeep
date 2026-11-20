/**
 * Making a project edit the way every Projects screen does
 * (`agent-history.md` §5): send it, say what changed in a notice with
 * Undo, or say it did not work. Undo takes back that one edit while
 * nothing was edited after it.
 */
import { useCallback } from 'react'
import { toast } from 'sonner'
import type { ProjectAction } from '@/lib/backend-client/agent-projects'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { commandErrorCode } from '@/lib/ipc/command-error'
import { editProject, undoProjectEdit } from '@/lib/queries/agent-projects'

export interface EditNotice {
  title: string
  description?: string
  /** Without Undo (a name saved from a field the person can change back). */
  noUndo?: boolean
}

export function useProjectEdit() {
  const { t } = useI18n()
  return useCallback(
    async (action: ProjectAction, notice: EditNotice | null = null) => {
      let result
      try {
        result = await editProject(action)
      } catch (error) {
        toast.error(t('agentProjects.toast.failed'), {
          description: describeError(error),
        })
        return null
      }
      if (notice) {
        const version = result.version
        toast(notice.title, {
          description: notice.description,
          action: notice.noUndo
            ? undefined
            : {
                label: t('agentProjects.toast.undo'),
                onClick: () =>
                  void undoProjectEdit(version).catch((error: unknown) =>
                    toast.error(
                      commandErrorCode(error) === 'stale_cursor'
                        ? t('agentProjects.toast.undoLate')
                        : t('agentProjects.toast.undoFailed'),
                      { description: describeError(error) },
                    ),
                  ),
              },
        })
      }
      return result
    },
    [t],
  )
}
