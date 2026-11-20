/**
 * "Export ▸" in the reader's ⋯ menu (`agent-history.md` §4.3): one
 * conversation to a file the system's save dialog picks.
 *
 * Responsibilities: one export at a time per reader; a notice while it
 * runs (the backend gives no progress and no way to stop once the place
 * is chosen, so it is a spinner, not a bar); the result with the file's
 * place and a way to show it; nothing at all when the dialog was
 * cancelled; the reason when it failed, in plain words for the cases a
 * person can act on. A conversation that changed meanwhile also refreshes
 * the reader's header.
 * Not responsible for: the menu itself (`more-menu.tsx`).
 */
import { useState } from 'react'
import { toast } from 'sonner'
import {
  agentChatsClient,
  type ChatExportFormat,
} from '@/lib/backend-client/agent-chats'
import { supportClient } from '@/lib/backend-client/support'
import { describeError } from '@/lib/errors'
import { useI18n, type MessageKey } from '@/lib/i18n'
import { commandErrorCode } from '@/lib/ipc/command-error'
import { revealLabel } from './reader-words'

export const EXPORT_FORMATS: readonly ChatExportFormat[] = [
  'markdown',
  'html',
  'jsonl',
  'originals',
]

export const formatName: Record<ChatExportFormat, MessageKey> = {
  markdown: 'agentReader.export.markdown',
  html: 'agentReader.export.html',
  jsonl: 'agentReader.export.jsonl',
  originals: 'agentReader.export.originals',
}

function reasonOf(error: unknown): MessageKey | null {
  switch (commandErrorCode(error)) {
    case 'stale_cursor':
      return 'agentReader.export.reasons.changed'
    case 'destination-unavailable':
      return 'agentReader.export.reasons.destination'
    case 'not_found':
      return 'agentReader.export.reasons.missing'
    default:
      return null
  }
}

export function useExportChat({
  id,
  publication,
  leaf,
  onStale,
}: {
  id: string
  /** The line the reader shows: Markdown and HTML follow it. */
  publication: string
  leaf: number | null
  onStale: () => void
}) {
  const { t, language } = useI18n()
  const [running, setRunning] = useState<ChatExportFormat | null>(null)

  const start = async (format: ChatExportFormat) => {
    if (running) return
    setRunning(format)
    const name = t(formatName[format])
    const notice = toast.loading(
      t('agentReader.export.running', { format: name }),
    )
    try {
      const saved = await agentChatsClient.exportConversation({
        id,
        format,
        publication,
        ...(leaf === null ? {} : { leaf }),
        locale: language,
      })
      if (!saved) {
        toast.dismiss(notice)
        return
      }
      toast.success(t('agentReader.export.done', { format: name }), {
        id: notice,
        description: saved.path,
        // Long enough to reach for "Show in Finder".
        duration: 10_000,
        action: {
          label: t(revealLabel()),
          onClick: () =>
            void supportClient.revealFile(saved.path).catch((error: unknown) =>
              toast.error(t('agentReader.menu.revealFileFailed'), {
                description: describeError(error),
              }),
            ),
        },
      })
    } catch (error) {
      if (commandErrorCode(error) === 'stale_cursor') onStale()
      const reason = reasonOf(error)
      toast.error(t('agentReader.export.failed'), {
        id: notice,
        description: reason ? t(reason) : describeError(error),
      })
    } finally {
      setRunning(null)
    }
  }

  return { running, start }
}
