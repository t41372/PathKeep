/**
 * The detail panel's note box. Edits save on their own after a pause, and any
 * pending edit is written when the panel moves to another page. Loading and
 * load errors are handled by `AnnotationSection`.
 *
 * Coming back to a page whose note is still being saved shows the note as the
 * archive had it before; when the save lands, the box takes the saved note,
 * unless something typed here is still waiting to be saved. "Waiting" is
 * tracked, not read off the text: a box typed back to the note it started
 * from looks unedited and would otherwise take the answer of the save it is
 * correcting. A save compares what the box shows with the note the archive
 * holds now, never with an older copy of its own, so an edit back to the note
 * the save replaced is written. Saves go one at a time, each with the box's
 * newest text, so their answers cannot arrive in another order and put an
 * older note back in the box; one at a time across visits to the page too
 * (`annotation-writes`), so a save left from the last visit cannot land after
 * a newer one.
 */
import { useQueryClient } from '@tanstack/react-query'
import { Check } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { Textarea } from '@/components/ui/textarea'
import { annotationsClient } from '@/lib/backend-client/annotations'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { cacheSavedField, cachedNotes, enqueueSave } from './annotation-writes'
import { invalidateSearches } from './queries'

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
  /** The archive's note this box last took; the box is unedited while it shows it. */
  const [base, setBase] = useState(initial)
  /** Typed here and not yet written: the archive's note must not replace it. */
  const [edited, setEdited] = useState(false)
  if (initial !== base) {
    setBase(initial)
    if (text === base && !edited) setText(initial)
  }
  const [state, setState] = useState<SaveState>('idle')
  // What the box shows and the archive's note, for saves that run after this render.
  const shown = useRef(text)
  const archived = useRef(base)
  useEffect(() => {
    shown.current = text
    archived.current = base
  }, [text, base])
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined)
  const fade = useRef<ReturnType<typeof setTimeout>>(undefined)

  const save = useCallback(() => {
    void enqueueSave('notes', url, async () => {
      const value = shown.current
      if (value === (cachedNotes(client, url) ?? archived.current)) {
        if (timer.current === undefined) setEdited(false)
        return
      }
      setState('saving')
      try {
        const annotation = await annotationsClient.setUrlNotes({
          url,
          notes: value,
          sourceProfile: profileId ?? null,
        })
        archived.current = annotation.notes
        if (timer.current === undefined && shown.current === archived.current) {
          setEdited(false)
        }
        // A read started before this write would land later with the old note.
        await client.cancelQueries({ queryKey: ['annotations', 'url', url] })
        cacheSavedField(client, url, 'notes', annotation)
        void client.invalidateQueries({ queryKey: ['stars', 'list'] })
        invalidateSearches(client)
        setState('saved')
        clearTimeout(fade.current)
        fade.current = setTimeout(() => setState('idle'), SAVED_VISIBLE_MS)
      } catch {
        setState('error')
      }
    })
  }, [client, profileId, url])

  useEffect(
    () => () => {
      clearTimeout(fade.current)
      if (timer.current) {
        clearTimeout(timer.current)
        save()
      }
    },
    [save],
  )

  const onChange = (value: string) => {
    setEdited(true)
    setText(value)
    clearTimeout(timer.current)
    timer.current = setTimeout(() => {
      timer.current = undefined
      save()
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
