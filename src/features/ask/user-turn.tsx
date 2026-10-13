/**
 * One question in the transcript, and editing it: the edited question is
 * sent in its place and the conversation goes on from there; what came after
 * it is dropped (the user's choice, 2026-10-12).
 */
import { Pencil } from 'lucide-react'
import { memo, useEffect, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import { useI18n } from '@/lib/i18n'
import type { ChatTurn } from './chat-model'

export const UserTurn = memo(function UserTurn({
  turn,
  canEdit,
  dropsLater,
  onEdit,
}: {
  turn: ChatTurn
  /** False while an answer streams or no service is set up. */
  canEdit: boolean
  /** Whether answers follow this question, which the edit drops. */
  dropsLater: boolean
  onEdit: (text: string) => void
}) {
  const { t } = useI18n()
  const [draft, setDraft] = useState<string | null>(null)
  const box = useRef<HTMLTextAreaElement>(null)
  const editing = draft !== null
  useEffect(() => {
    if (draft === null) return
    const field = box.current
    if (!field) return
    field.style.height = 'auto'
    field.style.height = `${Math.min(field.scrollHeight, 240)}px`
  }, [draft])
  // Focus once, when editing starts.
  useEffect(() => {
    if (editing) box.current?.focus()
  }, [editing])

  if (draft !== null) {
    const send = () => {
      if (!draft.trim()) return
      onEdit(draft)
      setDraft(null)
    }
    return (
      <div className="flex w-full max-w-[80%] flex-col gap-2 self-end rounded-[18px] border bg-popover px-4 py-3 shadow-card animate-rise">
        <textarea
          ref={box}
          value={draft}
          aria-label={t('ask.edit.label')}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Escape') {
              event.preventDefault()
              setDraft(null)
              return
            }
            if (
              event.key !== 'Enter' ||
              event.shiftKey ||
              event.nativeEvent.isComposing
            )
              return
            event.preventDefault()
            send()
          }}
          className="max-h-[240px] w-full resize-none bg-transparent leading-normal outline-none"
        />
        {dropsLater && (
          <p className="text-xs text-muted-foreground">
            {t('ask.edit.dropsLater')}
          </p>
        )}
        <div className="flex justify-end gap-2">
          <Button variant="ghost" size="sm" onClick={() => setDraft(null)}>
            {t('common.cancel')}
          </Button>
          <Button size="sm" disabled={!draft.trim()} onClick={send}>
            {t('ask.edit.send')}
          </Button>
        </div>
      </div>
    )
  }

  return (
    <div className="group flex max-w-[80%] items-start gap-1 self-end">
      {canEdit && (
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label={t('ask.edit.action')}
          onClick={() => setDraft(turn.content)}
          className="mt-2 opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100"
        >
          <Pencil />
        </Button>
      )}
      <div className="rounded-[18px] bg-user-bubble px-4 py-2.5 leading-normal whitespace-pre-wrap text-user-bubble-foreground animate-rise">
        {turn.content}
      </div>
    </div>
  )
})
