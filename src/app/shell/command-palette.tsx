/** ⌘K palette: jump to a screen, run an action, or find a visit. */
import { useQuery } from '@tanstack/react-query'
import {
  ChartColumn,
  HardDriveDownload,
  History,
  House,
  Lock,
  MessageCircle,
  MessageCirclePlus,
  Moon,
  RefreshCw,
  Search,
  Settings,
  Sun,
  type LucideIcon,
} from 'lucide-react'
import { useMemo, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandShortcut,
} from '@/components/ui/command'
import { Favicon } from '@/components/app/favicon'
import { explorerClient } from '@/lib/backend-client/explorer'
import { useDebouncedValue } from '@/lib/hooks/use-debounced-value'
import { useFormat, useI18n, type MessageKey } from '@/lib/i18n'
import { queryKeys } from '@/lib/query'
import { useTheme } from '@/lib/theme'
import { useBackupRunner } from '../backup-runner'

interface PaletteItem {
  id: string
  icon: LucideIcon
  label: string
  hint?: string
  run: () => void
}

const pages: { to: string; icon: LucideIcon; label: MessageKey }[] = [
  { to: '/', icon: House, label: 'shell.nav.home' },
  { to: '/history', icon: History, label: 'shell.nav.history' },
  { to: '/insights', icon: ChartColumn, label: 'shell.nav.insights' },
  { to: '/ask', icon: MessageCircle, label: 'shell.nav.ask' },
  { to: '/backup', icon: HardDriveDownload, label: 'shell.nav.backup' },
  { to: '/settings', icon: Settings, label: 'shell.nav.settings' },
]

export function CommandPalette({
  open,
  onOpenChange,
  onLock,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  onLock: () => void
}) {
  const { t } = useI18n()
  const format = useFormat()
  const navigate = useNavigate()
  const { resolved, setPreference } = useTheme()
  const backup = useBackupRunner()
  const [query, setQuery] = useState('')
  const term = useDebouncedValue(query.trim(), 180)

  const visits = useQuery({
    queryKey: [...queryKeys.archiveData, 'palette', term],
    // One row per page: a page visited ten times should not fill the list.
    queryFn: async () => {
      const page = await explorerClient.queryHistory({
        q: term,
        limit: 30,
        sort: 'relevance',
        includeTotal: false,
      })
      const seen = new Set<string>()
      return page.items
        .filter((visit) => !seen.has(visit.url) && seen.add(visit.url))
        .slice(0, 6)
    },
    enabled: open && term.length >= 2,
    staleTime: 30_000,
  })

  const close = () => {
    onOpenChange(false)
    setQuery('')
  }
  const go = (to: string) => {
    close()
    void navigate(to)
  }

  const groups = useMemo(() => {
    const needle = query.trim().toLowerCase()
    const matches = (label: string) =>
      !needle || label.toLowerCase().includes(needle)
    const pageItems: PaletteItem[] = pages.map((page) => ({
      id: `page:${page.to}`,
      icon: page.icon,
      label: t(page.label),
      run: () => go(page.to),
    }))
    const actionItems: PaletteItem[] = [
      {
        id: 'backup',
        icon: RefreshCw,
        label: t('shell.palette.backupNow'),
        run: () => (close(), void backup.run()),
      },
      {
        id: 'theme',
        icon: resolved === 'dark' ? Sun : Moon,
        label: t('shell.palette.toggleTheme'),
        run: () => (
          close(),
          setPreference(resolved === 'dark' ? 'light' : 'dark')
        ),
      },
      {
        id: 'lock',
        icon: Lock,
        label: t('shell.palette.lock'),
        hint: '⌘L',
        run: () => (close(), onLock()),
      },
      {
        id: 'chat',
        icon: MessageCirclePlus,
        label: t('shell.palette.newChat'),
        run: () => go('/ask?new=1'),
      },
    ]
    return {
      pages: pageItems.filter((item) => matches(item.label)),
      actions: actionItems.filter((item) => matches(item.label)),
    }
    // `go` and `close` only close over stable setters.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, t, resolved, backup.run, onLock])

  const searchTarget = `/history?q=${encodeURIComponent(query.trim())}`

  return (
    <CommandDialog
      open={open}
      onOpenChange={(next) => (next ? onOpenChange(true) : close())}
      title={t('shell.nav.palette')}
      description={t('shell.palette.placeholder')}
      shouldFilter={false}
      className="sm:max-w-xl"
    >
      <CommandInput
        value={query}
        onValueChange={setQuery}
        placeholder={t('shell.palette.placeholder')}
      />
      <CommandList className="max-h-[420px]">
        <CommandEmpty>{t('shell.palette.empty')}</CommandEmpty>
        {groups.pages.length > 0 && (
          <CommandGroup heading={t('shell.palette.pages')}>
            {groups.pages.map((item) => (
              <PaletteRow key={item.id} item={item} />
            ))}
          </CommandGroup>
        )}
        {groups.actions.length > 0 && (
          <CommandGroup heading={t('shell.palette.actions')}>
            {groups.actions.map((item) => (
              <PaletteRow key={item.id} item={item} />
            ))}
          </CommandGroup>
        )}
        {term.length >= 2 && (
          <CommandGroup heading={t('shell.palette.visits')}>
            {(visits.data ?? []).map((visit) => (
              <CommandItem
                key={visit.id}
                value={`visit:${visit.id}`}
                onSelect={() =>
                  go(`/history?q=${encodeURIComponent(term)}&visit=${visit.id}`)
                }
              >
                <Favicon
                  domain={visit.domain}
                  src={visit.favicon?.dataUrl}
                  className="size-4"
                />
                <span className="truncate">{visit.title || visit.url}</span>
                <CommandShortcut className="tracking-normal">
                  {format.dayAndTime(visit.visitedAt)}
                </CommandShortcut>
              </CommandItem>
            ))}
            <CommandItem
              value="search-history"
              onSelect={() => go(searchTarget)}
            >
              <Search />
              <span className="truncate">
                {t('shell.palette.searchHistory', { query: term })}
              </span>
            </CommandItem>
          </CommandGroup>
        )}
      </CommandList>
      <div className="flex gap-4 border-t px-3 py-2 text-[11px] text-muted-foreground">
        <span>↑↓ {t('shell.palette.hintNavigate')}</span>
        <span>↵ {t('shell.palette.hintOpen')}</span>
      </div>
    </CommandDialog>
  )
}

function PaletteRow({ item }: { item: PaletteItem }) {
  const Icon = item.icon
  return (
    <CommandItem value={item.id} onSelect={item.run}>
      <Icon />
      <span>{item.label}</span>
      {item.hint && <CommandShortcut>{item.hint}</CommandShortcut>}
    </CommandItem>
  )
}
