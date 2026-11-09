/**
 * Advanced search: a button in the search box that opens a form for the
 * full-text operators ("Words") and for a regex put together from pieces
 * ("Pattern"), so nobody has to remember the syntax. Search writes the
 * query into the search box with the mode it needs. What was typed stays
 * until History closes, so the form can be reopened to adjust a search.
 *
 * Responsible for: the popover, the open tab, the preview of what will be
 * searched, and applying it.
 * Not responsible for: writing queries or patterns (words-query.ts,
 * regex-pieces.ts), checking a regex (regex-dialect.ts) or running a search.
 */
import { SlidersHorizontal } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { useI18n } from '@/lib/i18n'
import type { SearchMode } from './history-params'
import { PatternFields } from './pattern-fields'
import {
  composePattern,
  editable,
  emptyPatternForm,
  type EditablePattern,
} from './regex-pieces'
import { checkRustRegex } from './regex-dialect'
import { RegexProblemText } from './results-line'
import { WordsFields } from './words-fields'
import {
  composeWordsQuery,
  emptyWordsForm,
  isPlainWords,
  type WordsForm,
} from './words-query'

type Tab = 'words' | 'pattern'

const freshPattern = (): EditablePattern => ({
  ...emptyPatternForm,
  pieces: editable(emptyPatternForm.pieces),
})

const isBlank = (form: WordsForm) =>
  Object.values(form).every((value) => value === '' || value === false)

/**
 * The search box text for a regex. The box trims what it holds and reads
 * `/…/` as "regex, without the slashes", so a pattern that starts or ends
 * with a space, or itself with slashes, is wrapped in a pair of slashes,
 * which the box removes again, keeping it exactly as composed.
 */
function regexQuery(pattern: string) {
  return /^\s|\s$|^\/.+\/$/s.test(pattern) ? `/${pattern}/` : pattern
}

export function SearchBuilder({
  query,
  mode,
  onApply,
}: {
  /** What the search box holds now. */
  query: string
  mode: SearchMode
  onApply: (query: string, mode: SearchMode) => void
}) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const [tab, setTab] = useState<Tab>('words')
  const [words, setWords] = useState(emptyWordsForm)
  const [pattern, setPattern] = useState(freshPattern)

  const composed =
    tab === 'words' ? composeWordsQuery(words) : composePattern(pattern)
  const problem =
    tab === 'pattern' && composed ? checkRustRegex(composed) : null

  const openChange = (next: boolean) => {
    if (next) {
      // Plain words already typed are where a search usually starts from.
      if (isBlank(words) && isPlainWords(query))
        setWords({ ...emptyWordsForm, all: query.trim() })
      if (mode === 'regex') setTab('pattern')
    }
    setOpen(next)
  }

  const apply = () => {
    if (!composed || problem) return
    if (tab === 'words') onApply(composed, 'full')
    else onApply(regexQuery(composed), 'regex')
    setOpen(false)
  }

  return (
    <Popover open={open} onOpenChange={openChange}>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label={t('historySearch.builder.button')}
          className="flex size-[22px] items-center justify-center rounded-full text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 data-[state=open]:text-foreground"
        >
          <SlidersHorizontal className="size-4" />
        </button>
      </PopoverTrigger>
      <PopoverContent
        align="end"
        sideOffset={10}
        aria-label={t('historySearch.builder.title')}
        className="w-[480px] p-0"
      >
        <form
          onSubmit={(event) => {
            event.preventDefault()
            apply()
          }}
        >
          <Tabs value={tab} onValueChange={(next) => setTab(next as Tab)}>
            <div className="flex items-center justify-between gap-3 border-b px-4 pt-3 pb-2.5">
              <h2 className="text-sm font-semibold">
                {t('historySearch.builder.title')}
              </h2>
              <TabsList className="h-8">
                {(['words', 'pattern'] as const).map((name) => (
                  <TabsTrigger
                    key={name}
                    value={name}
                    className="h-6.5 px-2.5 text-xs"
                  >
                    {t(`historySearch.builder.tabs.${name}`)}
                  </TabsTrigger>
                ))}
              </TabsList>
            </div>
            <div className="max-h-[min(460px,calc(100vh-280px))] overflow-y-auto px-4 py-3">
              <TabsContent value="words">
                <WordsFields form={words} onChange={setWords} />
              </TabsContent>
              <TabsContent value="pattern">
                <PatternFields form={pattern} onChange={setPattern} />
              </TabsContent>
            </div>
          </Tabs>
          <div className="flex flex-col gap-2 border-t px-4 py-3">
            <span className="text-[11px] text-muted-foreground">
              {t('historySearch.builder.preview')} ·{' '}
              {t(`history.modes.${tab === 'words' ? 'full' : 'regex'}`)}
            </span>
            <output
              aria-label={t('historySearch.builder.preview')}
              className="block min-h-[30px] rounded-md bg-muted px-2.5 py-1.5 font-mono text-[12px] break-all"
            >
              {composed || (
                <span className="font-sans text-muted-foreground">
                  {t('historySearch.builder.previewEmpty')}
                </span>
              )}
            </output>
            {problem && (
              <p className="text-xs">
                <RegexProblemText problem={problem} />
              </p>
            )}
            <div className="flex items-center justify-between gap-2">
              <span className="text-[11px] text-muted-foreground">
                {t('historySearch.builder.replaces')}
              </span>
              <div className="flex gap-1.5">
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={() =>
                    tab === 'words'
                      ? setWords(emptyWordsForm)
                      : setPattern(freshPattern())
                  }
                >
                  {t('historySearch.builder.clear')}
                </Button>
                <Button
                  type="submit"
                  size="sm"
                  disabled={!composed || problem !== null}
                >
                  {t('historySearch.builder.search')}
                </Button>
              </div>
            </div>
          </div>
        </form>
      </PopoverContent>
    </Popover>
  )
}
