/**
 * The message box: grows with the text, Enter sends, Shift+Enter breaks a
 * line. Below it, which model answers and how full the context is with what
 * is typed so far.
 */
import { ArrowUp, Square } from 'lucide-react'
import { useEffect, useRef, useState, type ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { useI18n } from '@/lib/i18n'
import type { ChatTurn } from './chat-model'
import { ContextMeter, ContextWarning } from './context-meter'
import { nextRequestTokens } from './context-usage'

/** What the meter needs: the model's context and what every request takes. */
export interface ComposerContext {
  turns: readonly ChatTurn[]
  fixedTokens: number
  contextLength: number | null
  service: string
}

const MAX_HEIGHT = 200

export function Composer({
  streaming,
  picker,
  context,
  onSend,
  onStop,
  focusKey,
  initialText = '',
}: {
  streaming: boolean
  /** Which model answers (`ModelPicker`). */
  picker: ReactNode
  /** `null` until the service has said how much the model reads. */
  context: ComposerContext | null
  onSend: (text: string) => void
  onStop: () => void
  /** Changes when a different chat is opened, so the box takes focus again. */
  focusKey: string
  /** What the box starts with: a question brought from elsewhere, not sent yet. */
  initialText?: string
}) {
  const { t } = useI18n()
  const [text, setText] = useState(initialText)
  const ref = useRef<HTMLTextAreaElement>(null)

  useEffect(() => {
    const box = ref.current
    if (!box) return
    box.style.height = 'auto'
    box.style.height = `${Math.min(box.scrollHeight, MAX_HEIGHT)}px`
  }, [text])

  useEffect(() => ref.current?.focus(), [focusKey])

  const canSend = text.trim().length > 0 && !streaming
  const used = context
    ? nextRequestTokens(context.fixedTokens, context.turns, text)
    : null
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
          title={t('ask.composer.hint')}
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
        {context && used !== null && (
          <ContextWarning used={used} contextLength={context.contextLength} />
        )}
        <div className="flex items-center gap-3">
          <div className="min-w-0 flex-1">{picker}</div>
          {context && used !== null && (
            <ContextMeter
              used={used}
              contextLength={context.contextLength}
              service={context.service}
            />
          )}
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
