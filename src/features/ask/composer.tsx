/** The message box: grows with the text, Enter sends, Shift+Enter breaks a line. */
import { ArrowUp, Square } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import { useI18n } from '@/lib/i18n'

const MAX_HEIGHT = 200

export function Composer({
  streaming,
  providerLabel,
  onSend,
  onStop,
  focusKey,
}: {
  streaming: boolean
  providerLabel: string
  onSend: (text: string) => void
  onStop: () => void
  /** Changes when a different chat is opened, so the box takes focus again. */
  focusKey: string
}) {
  const { t } = useI18n()
  const [text, setText] = useState('')
  const ref = useRef<HTMLTextAreaElement>(null)

  useEffect(() => {
    const box = ref.current
    if (!box) return
    box.style.height = 'auto'
    box.style.height = `${Math.min(box.scrollHeight, MAX_HEIGHT)}px`
  }, [text])

  useEffect(() => ref.current?.focus(), [focusKey])

  const canSend = text.trim().length > 0 && !streaming
  const submit = () => {
    if (!canSend) return
    onSend(text)
    setText('')
  }

  return (
    <div className="px-8 pb-5">
      <div className="mx-auto flex max-w-[780px] flex-col gap-2 rounded-2xl border bg-popover px-4 pt-3 pb-2.5 shadow-float">
        <textarea
          ref={ref}
          rows={1}
          value={text}
          aria-label={t('ask.composer.label')}
          placeholder={t('ask.composer.placeholder')}
          onChange={(event) => setText(event.target.value)}
          onKeyDown={(event) => {
            // Enter confirms an IME candidate; only a plain Enter sends.
            if (
              event.key !== 'Enter' ||
              event.shiftKey ||
              event.nativeEvent.isComposing
            )
              return
            event.preventDefault()
            submit()
          }}
          className="max-h-[200px] w-full resize-none bg-transparent leading-6 outline-none placeholder:text-muted-foreground"
        />
        <div className="flex items-center justify-between gap-3">
          <span
            title={t('ask.composer.hint')}
            className="min-w-0 truncate font-mono text-[11px] text-muted-foreground"
          >
            {t('ask.composer.provider', { provider: providerLabel })}
          </span>
          {streaming ? (
            <Button
              size="icon-sm"
              variant="secondary"
              aria-label={t('ask.composer.stop')}
              onClick={onStop}
              className="rounded-full"
            >
              <Square className="fill-current" />
            </Button>
          ) : (
            <Button
              size="icon-sm"
              aria-label={t('ask.composer.send')}
              disabled={!canSend}
              onClick={submit}
              className="rounded-full"
            >
              <ArrowUp />
            </Button>
          )}
        </div>
      </div>
    </div>
  )
}
