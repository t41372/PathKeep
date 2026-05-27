/**
 * ⌘K search palette.
 *
 * Why this file exists:
 * - Quick-jump search backed by the same `query_history` command the full
 *   Search page uses. The design positions ⌘K as a powerful *secondary* tool
 *   (chat #1: "main entry is contact-sheet, ⌘K is the auxiliary").
 *
 * Responsibilities:
 * - Open / close on parent control plus internal Escape handler.
 * - Debounce typed query into a backend keyword query.
 * - Render result rows linking into the Explorer (target date + entry id).
 * - Surface a "full search" affordance (⌘ + Enter) that routes to /search.
 *
 * Not responsible for:
 * - Driving backend itself — it consumes `onSearch` (query => Promise<rows>).
 */

import { useCallback, useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from '@/components/ui/command'
import { useI18n } from '@/lib/i18n/hooks'
import { cn } from '@/lib/cn'

export interface PaletteResult {
  id: string
  title: string
  domain: string
  url: string
  /** ISO date (yyyy-mm-dd) used to jump back into the Explorer. */
  visitDate: string | null
  visitTime: string | null
}

export interface PKSearchPaletteProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  onSearch: (query: string) => Promise<PaletteResult[]>
  onSelect: (result: PaletteResult) => void
}

export function PKSearchPalette({
  open,
  onOpenChange,
  onSearch,
  onSelect,
}: PKSearchPaletteProps) {
  const { t } = useI18n()
  const navigate = useNavigate()
  const [query, setQuery] = useState('')
  const [results, setResults] = useState<PaletteResult[]>([])
  const [busy, setBusy] = useState(false)

  const openValue = open ? 'open' : 'closed'
  useEffect(() => {
    setQuery('')
    setResults([])
    setBusy(false)
  }, [openValue])

  useEffect(() => {
    if (!open) return undefined
    const trimmed = query.trim()
    if (!trimmed) {
      return undefined
    }
    let cancelled = false
    setBusy(true)
    const handle = window.setTimeout(() => {
      void (async () => {
        try {
          const hits = await onSearch(trimmed)
          if (!cancelled) setResults(hits)
        } catch {
          if (!cancelled) setResults([])
        } finally {
          if (!cancelled) setBusy(false)
        }
      })()
    }, 160)
    return () => {
      cancelled = true
      window.clearTimeout(handle)
    }
  }, [query, open, onSearch])

  const handleFullSearch = useCallback(() => {
    const trimmed = query.trim()
    onOpenChange(false)
    void navigate(
      trimmed ? `/explorer?q=${encodeURIComponent(trimmed)}` : '/explorer',
    )
  }, [navigate, onOpenChange, query])

  return (
    <CommandDialog
      open={open}
      onOpenChange={onOpenChange}
      title={t('shell.paletteTitle')}
      description={t('shell.paletteDescription')}
      className="border border-border-default bg-paper"
    >
      <CommandInput
        placeholder={t('shell.findAPage')}
        value={query}
        onValueChange={setQuery}
        onKeyDown={(event) => {
          if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
            event.preventDefault()
            handleFullSearch()
          }
        }}
      />
      <CommandList>
        {!query.trim() ? (
          <CommandEmpty>
            <span className="font-serif text-ink-faint italic">
              {t('shell.paletteEmptyHint')}
            </span>
          </CommandEmpty>
        ) : busy ? (
          <CommandEmpty>
            <span className="font-mono text-[11px] text-ink-faint">
              {t('shell.paletteLoading')}
            </span>
          </CommandEmpty>
        ) : results.length === 0 ? (
          <CommandEmpty>
            <span className="font-serif text-ink-faint italic">
              {t('shell.paletteNoResults')}
            </span>
          </CommandEmpty>
        ) : (
          <CommandGroup>
            {results.map((result) => (
              <CommandItem
                key={result.id}
                value={`${result.title} ${result.domain}`}
                onSelect={() => {
                  onSelect(result)
                  onOpenChange(false)
                }}
                className="flex items-start gap-3"
              >
                <span
                  className={cn(
                    'mt-[2px] grid h-7 w-7 shrink-0 place-items-center font-mono text-[10px] font-semibold text-white',
                  )}
                  style={{ background: hashDomainColor(result.domain) }}
                >
                  {domainAbbreviation(result.domain)}
                </span>
                <span className="flex min-w-0 flex-col">
                  <span className="truncate font-serif text-[13px] text-ink">
                    {result.title}
                  </span>
                  <span className="truncate font-mono text-[10.5px] text-ink-faint">
                    {result.domain}
                    {result.visitDate ? ` · ${result.visitDate}` : ''}
                    {result.visitTime ? ` ${result.visitTime}` : ''}
                  </span>
                </span>
              </CommandItem>
            ))}
          </CommandGroup>
        )}
      </CommandList>
      <div className="border-border-light flex items-center gap-3 border-t px-3 py-2 font-mono text-[10px] text-ink-faint">
        <span>↵ {t('shell.paletteHintOpen')}</span>
        <span>⌘↵ {t('shell.paletteHintFullSearch')}</span>
        <span>↑↓ {t('shell.paletteHintNavigate')}</span>
        <span className="ml-auto">esc {t('shell.paletteHintClose')}</span>
      </div>
    </CommandDialog>
  )
}

function domainAbbreviation(domain: string): string {
  if (!domain) return '??'
  const cleaned = domain.replace(/^www\./, '')
  const root = cleaned.split('.')[0]
  if (root.length <= 2) return root.toUpperCase()
  return root.substring(0, 2).toUpperCase()
}

function hashDomainColor(domain: string): string {
  const palette = [
    '#24292e',
    '#4285f4',
    '#e87922',
    '#a8322d',
    '#cc0000',
    '#7b5b3a',
    '#cc4500',
    '#1a8967',
  ]
  let hash = 0
  for (let i = 0; i < domain.length; i += 1) {
    hash = (hash << 5) - hash + domain.charCodeAt(i)
    hash |= 0
  }
  return palette[Math.abs(hash) % palette.length]
}
