/**
 * "Export ▸" in the reader's ⋯ menu (`agent-history.md` §4.3): one
 * conversation to a file the system's save dialog picks.
 *
 * Responsibilities: one export at a time per reader; a notice while it
 * runs, with the bytes written so far (nobody knows the size before it is
 * written, so it is a count, not a bar) and Stop; saying it is stopping,
 * then that nothing was written and a file already there is untouched, or
 * that it was too late and the file is being put in place; the result with
 * the file's place and a way to show it; nothing at all when the dialog
 * was cancelled; the reason when it failed, in plain words for the cases a
 * person can act on. A conversation that changed meanwhile also refreshes
 * the reader's header.
 * Not responsible for: the menu itself (`more-menu.tsx`).
 */
import { useState } from 'react'
import { toast } from 'sonner'
import {
  AGENT_EXPORT_PROGRESS_EVENT,
  agentChatsClient,
  type ChatExportFormat,
  type ChatExportProgress,
} from '@/lib/backend-client/agent-chats'
import { supportClient } from '@/lib/backend-client/support'
import { describeError } from '@/lib/errors'
import { useFormat, useI18n, type MessageKey } from '@/lib/i18n'
import { commandErrorCode } from '@/lib/ipc/command-error'
import { listenDesktopEvent } from '@/lib/ipc/events'
import { revealLabel } from './reader-words'

/**
 * How often the notice takes a new count. Progress comes up to ten times a
 * second, and the notice is read out to screen readers.
 */
const SHOW_EVERY_MS = 500

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
  const f = useFormat()
  const [running, setRunning] = useState<ChatExportFormat | null>(null)

  const start = async (format: ChatExportFormat) => {
    if (running) return
    setRunning(format)
    const name = t(formatName[format])
    const exportId = crypto.randomUUID()
    let written: number | null = null
    let shownAt = 0
    let phase: 'writing' | 'stopping' | 'tooLate' = 'writing'

    const show = () => {
      shownAt = Date.now()
      if (phase === 'stopping') {
        toast.loading(t('agentReader.export.stopping'), {
          id: notice,
          description: undefined,
          action: undefined,
        })
        return
      }
      toast.loading(t('agentReader.export.running', { format: name }), {
        id: notice,
        description:
          phase === 'tooLate'
            ? t('agentReader.export.tooLate')
            : written === null
              ? undefined
              : t('agentReader.export.written', { size: f.bytes(written) }),
        action:
          phase === 'writing'
            ? {
                label: t('agentReader.export.stop'),
                onClick: (event) => {
                  // The notice stays: it says it is stopping, then how it ended.
                  event.preventDefault()
                  void stop()
                },
              }
            : undefined,
      })
    }
    const stop = async () => {
      if (phase !== 'writing') return
      phase = 'stopping'
      show()
      try {
        const answer = await agentChatsClient.cancelExport(exportId)
        // Not taken: the file is being put in place, or the export ended.
        if (!answer.cancelled) {
          phase = 'tooLate'
          show()
        }
      } catch (error) {
        phase = 'writing'
        show()
        toast.error(t('agentReader.export.stopFailed'), {
          description: describeError(error),
        })
      }
    }

    const notice = toast.loading(
      t('agentReader.export.running', { format: name }),
    )
    show()
    const unlisten = await listenDesktopEvent<ChatExportProgress>(
      AGENT_EXPORT_PROGRESS_EVENT,
      (event) => {
        if (event.exportId !== exportId) return
        written = Number(event.bytesWritten)
        if (phase !== 'stopping' && Date.now() - shownAt >= SHOW_EVERY_MS)
          show()
      },
    )
    try {
      const saved = await agentChatsClient.exportConversation({
        id,
        exportId,
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
      const code = commandErrorCode(error)
      if (code === 'cancelled') {
        toast(t('agentReader.export.stopped'), {
          id: notice,
          description: t('agentReader.export.stoppedBody'),
          action: undefined,
        })
        return
      }
      if (code === 'stale_cursor') onStale()
      const reason = reasonOf(error)
      toast.error(t('agentReader.export.failed'), {
        id: notice,
        description: reason ? t(reason) : describeError(error),
        action: undefined,
      })
    } finally {
      unlisten()
      setRunning(null)
    }
  }

  return { running, start }
}
