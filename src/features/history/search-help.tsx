/**
 * The search box's cheat sheet: a "?" button that opens (on click, focus +
 * Enter, or a short hover) a list of the operators the backend parses, each
 * with an example that fills the search box when picked.
 *
 * Responsible for: the examples and their wording.
 * Not responsible for: parsing; `vault-core/src/archive/search_query.rs` is
 * the source of truth, and every operator listed here is one it handles.
 */
import { CircleHelp } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { localDateKey } from '@/lib/backend-client/insights'
import { useI18n, type MessageKey } from '@/lib/i18n'

const HOVER_OPEN_MS = 350
const HOVER_CLOSE_MS = 200

/** Last month's first and last day, so the date example always finds something recent. */
function lastMonth(now = new Date()) {
  const first = new Date(now.getFullYear(), now.getMonth() - 1, 1)
  const last = new Date(now.getFullYear(), now.getMonth(), 0)
  return `after:${localDateKey(first)} before:${localDateKey(last)}`
}

const examples: { query: string | (() => string); help: MessageKey }[] = [
  { query: 'site:docs.rs tokio', help: 'historySearch.help.site' },
  { query: 'tokio -smol', help: 'historySearch.help.exclude' },
  { query: '"half marathon"', help: 'historySearch.help.phrase' },
  { query: 'tokio OR tauri', help: 'historySearch.help.or' },
  { query: 'intitle:runtime', help: 'historySearch.help.intitle' },
  { query: 'inurl:issues', help: 'historySearch.help.inurl' },
  { query: 'filetype:html', help: 'historySearch.help.filetype' },
  { query: lastMonth, help: 'historySearch.help.dates' },
  { query: 'tag:reading', help: 'historySearch.help.tag' },
  { query: 'note:rewrite', help: 'historySearch.help.note' },
  { query: '/item\\?id=\\d+/', help: 'historySearch.help.regex' },
]

export function SearchHelp({ onPick }: { onPick: (query: string) => void }) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  // Opened by click or keyboard: stays until dismissed, hover does not close it.
  const pinned = useRef(false)
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined)
  useEffect(() => () => clearTimeout(timer.current), [])

  const hoverOpen = () => {
    clearTimeout(timer.current)
    if (!open) timer.current = setTimeout(() => setOpen(true), HOVER_OPEN_MS)
  }
  const hoverClose = () => {
    clearTimeout(timer.current)
    if (!pinned.current)
      timer.current = setTimeout(() => setOpen(false), HOVER_CLOSE_MS)
  }

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        clearTimeout(timer.current)
        pinned.current = next
        setOpen(next)
      }}
    >
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label={t('historySearch.help.button')}
          onPointerEnter={hoverOpen}
          onPointerLeave={hoverClose}
          className="flex size-[22px] items-center justify-center rounded-full text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 data-[state=open]:text-foreground"
        >
          <CircleHelp className="size-4" />
        </button>
      </PopoverTrigger>
      <PopoverContent
        align="end"
        sideOffset={10}
        onPointerEnter={() => clearTimeout(timer.current)}
        onPointerLeave={hoverClose}
        // Keep focus in the search box when the sheet opens on hover.
        onOpenAutoFocus={(event) => {
          if (!pinned.current) event.preventDefault()
        }}
        className="w-[380px] p-0"
      >
        <div className="flex flex-col gap-0.5 border-b px-4 pt-3.5 pb-3">
          <h2 className="text-sm font-semibold">
            {t('historySearch.help.title')}
          </h2>
          <p className="text-xs text-muted-foreground">
            {t('historySearch.help.intro')}
          </p>
        </div>
        <ul className="flex max-h-[min(420px,60vh)] flex-col overflow-y-auto p-1.5">
          {examples.map(({ query, help }) => {
            const text = typeof query === 'function' ? query() : query
            return (
              <li key={help}>
                <button
                  type="button"
                  onClick={() => {
                    pinned.current = false
                    setOpen(false)
                    onPick(text)
                  }}
                  className="flex w-full flex-col gap-0.5 rounded-md px-2.5 py-1.5 text-left outline-none hover:bg-muted focus-visible:bg-muted focus-visible:ring-2 focus-visible:ring-ring/50"
                >
                  <code className="font-mono text-[12px] text-foreground">
                    {text}
                  </code>
                  <span className="text-xs text-muted-foreground">
                    {t(help)}
                  </span>
                </button>
              </li>
            )
          })}
        </ul>
        <p className="border-t px-4 py-2.5 text-[11px] leading-relaxed text-muted-foreground">
          {t('historySearch.help.footer')}
        </p>
      </PopoverContent>
    </Popover>
  )
}
