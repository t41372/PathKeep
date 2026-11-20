/**
 * The reader's find bar (`agent-history.md` §4.1, §4.6): what to find,
 * which occurrence of how many (counting up while a long conversation is
 * searched), previous and next, and close. Enter goes to the next one,
 * Shift+Enter to the one before, Esc closes the bar and nothing else.
 */
import { ChevronDown, ChevronUp, X } from 'lucide-react'
import { useEffect, useRef } from 'react'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/cn'
import { useFormat, useI18n } from '@/lib/i18n'
import type { ReaderFind } from './use-reader-find'

export function FindBar({
  find,
  className,
}: {
  find: ReaderFind
  className?: string
}) {
  const { t } = useI18n()
  const format = useFormat()
  const input = useRef<HTMLInputElement>(null)
  const { focusNonce, selectedRef } = find
  useEffect(() => {
    const element = input.current
    if (!element) return
    element.focus()
    if (selectedRef.current !== focusNonce) {
      // ⌘F: what was there is selected, to type over.
      selectedRef.current = focusNonce
      element.select()
    } else {
      // Drawn again (the sheet opened): carry on typing where it was.
      element.setSelectionRange(element.value.length, element.value.length)
    }
  }, [focusNonce, selectedRef])
  const total = find.state.matches.items.length
  const finding = find.state.phase === 'finding'
  const count =
    find.state.phase === 'failed'
      ? find.state.error === 'invalid_regex'
        ? t('agentReader.find.badRegex')
        : t('agentReader.find.failed')
      : !find.query
        ? ''
        : total === 0
          ? finding
            ? t('agentReader.find.finding')
            : t('agentReader.find.none')
          : t('agentReader.find.count', {
              current:
                find.current === null ? '–' : format.number(find.current + 1),
              total: format.number(total),
            }) + (finding ? '…' : '')
  return (
    <div
      role="search"
      aria-label={t('agentReader.find.label')}
      data-find-bar=""
      className={cn(
        'flex items-center gap-1.5 border-b bg-card/60 px-4 py-1.5',
        className,
      )}
    >
      <Input
        ref={input}
        value={find.draft}
        aria-label={t('agentReader.find.label')}
        placeholder={t('agentReader.find.placeholder')}
        onChange={(event) => find.type(event.target.value)}
        onCompositionStart={() => find.composing(true)}
        onCompositionEnd={(event) => {
          find.composing(false)
          find.type(event.currentTarget.value)
        }}
        onKeyDown={(event) => {
          if (event.nativeEvent.isComposing) return
          if (event.key === 'Enter') {
            event.preventDefault()
            if (event.shiftKey) find.previous()
            else find.next()
          } else if (event.key === 'Escape') {
            event.preventDefault()
            event.stopPropagation()
            find.close()
          }
        }}
        className="h-7 max-w-[320px] flex-1 text-[13px]"
      />
      <span
        aria-live="polite"
        className="min-w-[64px] text-[12px] text-muted-foreground tabular"
      >
        {count}
      </span>
      <button
        type="button"
        aria-label={t('agentReader.find.previous')}
        disabled={total === 0}
        onClick={find.previous}
        className="flex size-7 items-center justify-center rounded-md text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-40"
      >
        <ChevronUp className="size-4" aria-hidden />
      </button>
      <button
        type="button"
        aria-label={t('agentReader.find.next')}
        disabled={total === 0}
        onClick={find.next}
        className="flex size-7 items-center justify-center rounded-md text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-40"
      >
        <ChevronDown className="size-4" aria-hidden />
      </button>
      <button
        type="button"
        aria-label={t('agentReader.find.close')}
        onClick={find.close}
        className="ml-auto flex size-7 items-center justify-center rounded-md text-muted-foreground hover:bg-muted hover:text-foreground"
      >
        <X className="size-4" aria-hidden />
      </button>
    </div>
  )
}
