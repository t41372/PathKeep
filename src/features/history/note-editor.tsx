/**
 * The detail panel's note box. Edits save on their own after a pause, and any
 * pending edit is written when the panel moves to another page. Loading and
 * load errors are handled by `AnnotationSection`.
 */
import { useQueryClient } from '@tanstack/react-query'
import { Check } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { Textarea } from '@/components/ui/textarea'
import { annotationsClient } from '@/lib/backend-client/annotations'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'

const SAVE_DELAY_MS = 800
const SAVED_VISIBLE_MS = 2000

type SaveState = 'idle' | 'saving' | 'saved' | 'error'

export function NoteField({
  url,
  profileId,
  initial,
}: {
  url: string
  profileId?: string
  initial: string
}) {
  const { t } = useI18n()
  const client = useQueryClient()
  const [text, setText] = useState(initial)
  const [state, setState] = useState<SaveState>('idle')
  const latest = useRef(initial)
  const saved = useRef(initial)
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined)
  const fade = useRef<ReturnType<typeof setTimeout>>(undefined)

  const save = useCallback(async () => {
    const value = latest.current
    if (value === saved.current) return
    setState('saving')
    try {
      const annotation = await annotationsClient.setUrlNotes({
        url,
        notes: value,
        sourceProfile: profileId ?? null,
      })
      saved.current = value
      client.setQueryData(['annotations', 'url', url], annotation)
      void client.invalidateQueries({ queryKey: ['annotations', 'list'] })
      setState('saved')
      clearTimeout(fade.current)
      fade.current = setTimeout(() => setState('idle'), SAVED_VISIBLE_MS)
    } catch {
      setState('error')
    }
  }, [client, profileId, url])

  useEffect(
    () => () => {
      clearTimeout(fade.current)
      if (timer.current) {
        clearTimeout(timer.current)
        void save()
      }
    },
    [save],
  )

  const onChange = (value: string) => {
    setText(value)
    latest.current = value
    clearTimeout(timer.current)
    timer.current = setTimeout(() => {
      timer.current = undefined
      void save()
    }, SAVE_DELAY_MS)
  }

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center justify-between">
        <label htmlFor="history-note" className="text-xs text-muted-foreground">
          {t('history.detail.note')}
        </label>
        <span
          aria-live="polite"
          className={cn(
            'flex items-center gap-1 text-xs text-muted-foreground transition-opacity duration-300',
            state === 'idle' ? 'opacity-0' : 'opacity-100',
            state === 'error' && 'text-destructive',
          )}
        >
          {state === 'saved' && <Check className="size-3" aria-hidden />}
          {state === 'saving' && t('history.detail.noteSaving')}
          {state === 'saved' && t('history.detail.noteSaved')}
          {state === 'error' && t('history.detail.noteFailed')}
        </span>
      </div>
      <Textarea
        id="history-note"
        value={text}
        onChange={(event) => onChange(event.target.value)}
        placeholder={t('history.detail.notePlaceholder')}
        className="min-h-[72px] resize-y bg-card"
      />
    </div>
  )
}
