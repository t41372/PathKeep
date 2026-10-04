/**
 * The detail panel's tags: chips with a remove button, and an input that adds
 * a tag on Enter or comma. Clicking a chip filters History to that tag.
 *
 * Keyboard: Backspace in the empty input moves to the last chip; Backspace or
 * Delete on a chip removes it; ←/→ move between chips and the input.
 *
 * Every change saves the whole set (`replace_url_tags`). Writes run one after
 * another, so the archive ends up with the last set the user saw.
 */
import { useQueryClient } from '@tanstack/react-query'
import { Hash, X } from 'lucide-react'
import { useCallback, useRef, useState, type KeyboardEvent } from 'react'
import { annotationsClient } from '@/lib/backend-client/annotations'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { addTags, type TagProblem } from './tags'

export function TagField({
  url,
  profileId,
  initial,
  onFilter,
}: {
  url: string
  profileId?: string
  initial: readonly string[]
  onFilter: (tag: string) => void
}) {
  const { t } = useI18n()
  const client = useQueryClient()
  const [tags, setTags] = useState<string[]>(() => [...initial])
  const [draft, setDraft] = useState('')
  const [problem, setProblem] = useState<TagProblem | 'saveFailed' | null>(null)
  const saved = useRef<string[]>([...initial])
  const chain = useRef(Promise.resolve())
  const input = useRef<HTMLInputElement>(null)
  const chips = useRef<(HTMLButtonElement | null)[]>([])

  const commit = useCallback(
    (next: string[]) => {
      setTags(next)
      setProblem(null)
      chain.current = chain.current.then(async () => {
        try {
          const annotation = await annotationsClient.replaceUrlTags({
            url,
            tags: next,
            sourceProfile: profileId ?? null,
          })
          saved.current = annotation.tags
          client.setQueryData(['annotations', 'url', url], annotation)
          void client.invalidateQueries({ queryKey: ['annotations', 'list'] })
        } catch {
          setTags(saved.current)
          setProblem('saveFailed')
        }
      })
    },
    [client, profileId, url],
  )

  const add = () => {
    const result = addTags(tags, draft)
    if ('problem' in result) {
      setProblem(result.problem)
      return
    }
    setDraft('')
    if (result.tags.length !== tags.length) commit(result.tags)
  }

  const remove = (index: number) => {
    commit(tags.filter((_, at) => at !== index))
    // Keep focus nearby: the previous chip, or the input when none is left.
    requestAnimationFrame(() => {
      const target = chips.current[Math.max(0, index - 1)]
      if (index > 0 && target) target.focus()
      else input.current?.focus()
    })
  }

  const onInputKey = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'Enter' || event.key === ',') {
      event.preventDefault()
      add()
    } else if (event.key === 'Backspace' && draft === '' && tags.length > 0) {
      event.preventDefault()
      chips.current[tags.length - 1]?.focus()
    } else if (
      event.key === 'ArrowLeft' &&
      event.currentTarget.selectionStart === 0 &&
      tags.length > 0
    ) {
      event.preventDefault()
      chips.current[tags.length - 1]?.focus()
    } else if (event.key === 'Escape' && draft) {
      setDraft('')
    }
  }

  const onChipKey = (
    event: KeyboardEvent<HTMLButtonElement>,
    index: number,
  ) => {
    if (event.key === 'Backspace' || event.key === 'Delete') {
      event.preventDefault()
      remove(index)
    } else if (event.key === 'ArrowLeft') {
      event.preventDefault()
      chips.current[Math.max(0, index - 1)]?.focus()
    } else if (event.key === 'ArrowRight') {
      event.preventDefault()
      const next = chips.current[index + 1]
      if (index + 1 < tags.length && next) next.focus()
      else input.current?.focus()
    }
  }

  const problemText = problem && t(`historyPage.tags.${problem}`)

  return (
    <div className="flex flex-col gap-1.5">
      <label
        htmlFor="history-tag-input"
        className="text-xs text-muted-foreground"
      >
        {t('historyPage.tags.label')}
      </label>
      <div
        className={cn(
          'flex min-h-9 flex-wrap items-center gap-1.5 rounded-md border bg-card px-2 py-1.5 transition-[color,box-shadow] focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/50',
          problem && 'border-destructive',
        )}
        onClick={(event) => {
          if (event.target === event.currentTarget) input.current?.focus()
        }}
      >
        {tags.map((tag, index) => (
          <span
            key={tag.toLowerCase()}
            className="inline-flex h-6 max-w-full animate-rise items-center rounded-md bg-brand-soft text-[12px] text-foreground"
          >
            <button
              ref={(node) => {
                chips.current[index] = node
              }}
              type="button"
              title={t('historyPage.tags.filter', { tag })}
              aria-label={t('historyPage.tags.filter', { tag })}
              onClick={() => onFilter(tag)}
              onKeyDown={(event) => onChipKey(event, index)}
              className="flex h-full min-w-0 items-center gap-0.5 rounded-l-md pr-1 pl-1.5 outline-none hover:text-brand focus-visible:ring-2 focus-visible:ring-ring/60"
            >
              <Hash className="size-3 shrink-0 opacity-60" aria-hidden />
              <span className="truncate">{tag}</span>
            </button>
            <button
              type="button"
              aria-label={t('historyPage.tags.remove', { tag })}
              onClick={() => remove(index)}
              className="flex h-full items-center rounded-r-md pr-1 pl-0.5 text-muted-foreground outline-none hover:text-foreground"
            >
              <X className="size-3" />
            </button>
          </span>
        ))}
        <input
          ref={input}
          id="history-tag-input"
          value={draft}
          onChange={(event) => {
            setDraft(event.target.value)
            if (problem && problem !== 'saveFailed') setProblem(null)
          }}
          onKeyDown={onInputKey}
          onBlur={() => draft.trim() && add()}
          placeholder={t(
            tags.length > 0
              ? 'historyPage.tags.placeholderMore'
              : 'historyPage.tags.placeholder',
          )}
          aria-describedby={problem ? 'history-tag-problem' : undefined}
          aria-invalid={problem ? true : undefined}
          className="h-6 min-w-[96px] flex-1 bg-transparent text-[13px] outline-none placeholder:text-muted-foreground"
        />
      </div>
      {problemText && (
        <p
          id="history-tag-problem"
          role="alert"
          className="text-xs text-destructive"
        >
          {problemText}
        </p>
      )}
    </div>
  )
}
