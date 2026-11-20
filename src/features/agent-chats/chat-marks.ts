/**
 * The marks a person puts on one conversation, from the list's right-click
 * menu and the reader alike (`agent-history.md` §8): star, put away and
 * bring back, move to the trash and put back, rename. Neither put away nor
 * the trash asks first: both only hide the conversation, and their notice
 * offers Undo, which is the opposite mark.
 *
 * Responsibilities: sending the mark, its notice and Undo, and saying so
 * when it failed. Not responsible for: what the list or the reader do
 * after (closing a reader whose conversation went to the trash is the
 * caller's `onTrashed`), or keeping caches current (`useEditChat`).
 */
import { useMemo } from 'react'
import { toast } from 'sonner'
import type { ChatEditRequest } from '@/lib/backend-client/agent-chats'
import { useI18n } from '@/lib/i18n'
import { useEditChat } from '@/lib/queries/agent-chats'

export function useChatMarks() {
  const { t } = useI18n()
  const edit = useEditChat()
  return useMemo(() => {
    const send = async (request: ChatEditRequest) => {
      try {
        return await edit(request)
      } catch (error) {
        toast.error(t('agentChats.toast.failed'), {
          description: error instanceof Error ? error.message : String(error),
        })
        return null
      }
    }
    const undo = (request: ChatEditRequest) => ({
      label: t('agentChats.toast.undo'),
      onClick: () => void send(request),
    })
    return {
      star: (id: string, on: boolean) => send({ action: 'star', id, on }),
      putAway: async (id: string) => {
        if (!(await send({ action: 'archive', id, on: true }))) return
        toast(t('agentChats.toast.putAway'), {
          description: t('agentChats.toast.putAwayBody'),
          action: undo({ action: 'archive', id, on: false }),
        })
      },
      bringBack: async (id: string) => {
        if (await send({ action: 'archive', id, on: false }))
          toast(t('agentChats.toast.broughtBack'))
      },
      trash: async (id: string, onTrashed?: () => void) => {
        if (!(await send({ action: 'trash', id }))) return
        onTrashed?.()
        toast(t('agentChats.toast.trashed'), {
          action: undo({ action: 'restore', id }),
        })
      },
      putBack: async (id: string) => {
        if (await send({ action: 'restore', id }))
          toast(t('agentChats.toast.putBack'))
      },
      /** An empty title goes back to the one the tool gave. */
      rename: (id: string, title: string) =>
        send({ action: 'rename', id, title }),
    }
  }, [edit, t])
}

export type ChatMarks = ReturnType<typeof useChatMarks>
