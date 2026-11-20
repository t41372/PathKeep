/**
 * The Records search box (`agent-history.md` §3.2): it sends the search
 * 280 ms after typing stops (never while an input method is composing),
 * at once on Enter, and clears it on Esc; `/` anywhere outside a text
 * field comes here, and so does ⌘F / Ctrl+F while no conversation is open
 * (with one open, ⌘F finds in it). A search sent from an empty box is a
 * new history entry; changing it after that is not. In semantic mode the
 * placeholder asks for a description instead of words.
 */
import { Search, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { Input } from '@/components/ui/input'
import { useI18n } from '@/lib/i18n'

const DEBOUNCE_MS = 280

function isTyping(target: EventTarget | null) {
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable ||
      ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName))
  )
}

export function SearchBox({
  value,
  scoped,
  semantic,
  readerOpen,
  onCommit,
}: {
  /** The search in the URL. */
  value: string | null
  /** On a project's page: "search this project". */
  scoped: boolean
  /** Searches by meaning (`mode=semantic`). */
  semantic: boolean
  readerOpen: boolean
  onCommit: (q: string | null, options: { push: boolean }) => void
}) {
  const { t } = useI18n()
  const input = useRef<HTMLInputElement>(null)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const composing = useRef(false)
  const [draft, setDraft] = useState(value ?? '')
  // Back, Forward or a link changed the search: show it. The search this
  // box sent coming back is not a change: what was typed since stays.
  const [shown, setShown] = useState(value)
  const [sent, setSent] = useState<string | null | undefined>(undefined)
  if (shown !== value) {
    setShown(value)
    if (value !== sent) setDraft(value ?? '')
  }

  const commit = (text: string) => {
    if (timer.current) clearTimeout(timer.current)
    timer.current = null
    const next = text.trim() ? text : null
    if (next === value) return
    setSent(next)
    onCommit(next, { push: value === null && next !== null })
  }
  const later = (text: string) => {
    if (timer.current) clearTimeout(timer.current)
    timer.current = setTimeout(() => commit(text), DEBOUNCE_MS)
  }

  useEffect(
    () => () => {
      if (timer.current) clearTimeout(timer.current)
    },
    [],
  )

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return
      const find =
        (event.metaKey || event.ctrlKey) &&
        !event.altKey &&
        event.key.toLowerCase() === 'f'
      if (find && !readerOpen) {
        event.preventDefault()
        input.current?.focus()
        input.current?.select()
        return
      }
      if (
        event.key === '/' &&
        !event.metaKey &&
        !event.ctrlKey &&
        !event.altKey &&
        !isTyping(event.target)
      ) {
        event.preventDefault()
        input.current?.focus()
        input.current?.select()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [readerOpen])

  return (
    <div className="relative flex min-w-0 flex-1 items-center">
      <Search
        className="pointer-events-none absolute left-2.5 size-4 text-muted-foreground"
        aria-hidden
      />
      <Input
        ref={input}
        type="search"
        value={draft}
        aria-label={t('agentSearch.box.label')}
        placeholder={t(
          semantic
            ? scoped
              ? 'agentSearch.box.placeholderSemanticProject'
              : 'agentSearch.box.placeholderSemantic'
            : scoped
              ? 'agentSearch.box.placeholderProject'
              : 'agentSearch.box.placeholder',
        )}
        onChange={(event) => {
          setDraft(event.target.value)
          if (!composing.current) later(event.target.value)
        }}
        onCompositionStart={() => {
          composing.current = true
          if (timer.current) clearTimeout(timer.current)
        }}
        onCompositionEnd={(event) => {
          composing.current = false
          later(event.currentTarget.value)
        }}
        onKeyDown={(event) => {
          if (event.nativeEvent.isComposing) return
          if (event.key === 'Enter') {
            event.preventDefault()
            commit(draft)
          } else if (event.key === 'Escape' && draft) {
            // Only the search: the reader beside it stays open.
            event.preventDefault()
            event.stopPropagation()
            setDraft('')
            commit('')
          }
        }}
        className="h-9 rounded-lg bg-card pr-8 pl-8 text-[13px] [&::-webkit-search-cancel-button]:hidden"
      />
      <span className="absolute right-2 flex items-center">
        {draft ? (
          <button
            type="button"
            aria-label={t('agentSearch.box.clear')}
            onClick={() => {
              setDraft('')
              commit('')
              input.current?.focus()
            }}
            className="flex size-5 items-center justify-center rounded-full bg-muted text-muted-foreground hover:text-foreground"
          >
            <X className="size-3" aria-hidden />
          </button>
        ) : (
          <kbd
            aria-hidden
            className="pointer-events-none rounded-[5px] border px-1.5 font-mono text-[11px] text-muted-foreground"
          >
            /
          </kbd>
        )}
      </span>
    </div>
  )
}
