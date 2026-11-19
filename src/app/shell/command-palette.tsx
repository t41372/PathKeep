/**
 * ⌘K palette: jump to a page, run an action, switch spaces, or find a
 * record. Groups, in order:
 * - Go to: every page of every space that is on, written
 *   `{space} · {page}`, then Ask and Settings;
 * - Actions;
 * - Switch to: the other spaces, when two or more are on;
 * - from two typed characters, each space's own results (registry
 *   `palette`), ending in a row that searches its list page for the words.
 *
 * Not responsible for: what a space's results are or how they are found;
 * each space brings its own.
 */
import {
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
import { Suspense, useMemo, useState } from 'react'
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
import { useDebouncedValue } from '@/lib/hooks/use-debounced-value'
import { useI18n } from '@/lib/i18n'
import {
  spaceKeyLabel,
  spacePagePath,
  spaceSearchPath,
  useSpacesOn,
  type Space,
} from '@/lib/spaces'
import { useTheme } from '@/lib/theme'
import { useBackupRunner } from '../backup-runner'
import { useCurrentSpace, useSwitchSpace } from './current-space'
import { SpaceTile } from './space-switcher'
import { shortcutLabel } from '@/lib/runtime'

interface PaletteItem {
  id: string
  icon: LucideIcon
  label: string
  hint?: string
  run: () => void
}

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
  const navigate = useNavigate()
  const { resolved, setPreference } = useTheme()
  const backup = useBackupRunner()
  const current = useCurrentSpace()
  const switchTo = useSwitchSpace()
  const [query, setQuery] = useState('')
  const term = useDebouncedValue(query.trim(), 180)
  const on = useSpacesOn()

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
    const pageItems: PaletteItem[] = [
      ...on.flatMap((space) =>
        space.pages.map((page) => {
          const to = spacePagePath(space, page)
          return {
            id: `page:${to}`,
            icon: page.icon,
            label: t('spaces.palette.page', {
              // The short name: "Browsing · History" reads better than
              // repeating the page's word after the full name.
              space: t(space.shortName),
              page: t(page.label),
            }),
            run: () => go(to),
          }
        }),
      ),
      {
        id: 'page:/ask',
        icon: MessageCircle,
        label: t('shell.nav.ask'),
        run: () => go('/ask'),
      },
      {
        id: 'page:/settings',
        icon: Settings,
        label: t('shell.nav.settings'),
        run: () => go('/settings'),
      },
    ]
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
        hint: shortcutLabel('L'),
        run: () => (close(), onLock()),
      },
      {
        id: 'chat',
        icon: MessageCirclePlus,
        label: t('shell.palette.newChat'),
        run: () => go('/ask?new=1'),
      },
    ]
    const others =
      on.length > 1
        ? on
            .map((space, index) => ({ space, index }))
            .filter(
              ({ space }) => space.id !== current.id && matches(t(space.name)),
            )
        : []
    return {
      pages: pageItems.filter((item) => matches(item.label)),
      actions: actionItems.filter((item) => matches(item.label)),
      others,
    }
    // `go` and `close` only close over stable setters.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, t, resolved, backup.run, onLock, current.id, on])

  return (
    <CommandDialog
      open={open}
      onOpenChange={(next) => (next ? onOpenChange(true) : close())}
      title={t('shell.nav.palette', { keys: shortcutLabel('K') })}
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
        {groups.others.length > 0 && (
          <CommandGroup heading={t('spaces.palette.switchTo')}>
            {groups.others.map(({ space, index }) => (
              <CommandItem
                key={space.id}
                value={`space:${space.id}`}
                onSelect={() => (close(), switchTo(space))}
              >
                <SpaceTile space={space} size="sm" />
                <span>{t(space.name)}</span>
                {index < 9 && (
                  <CommandShortcut>{spaceKeyLabel(index + 1)}</CommandShortcut>
                )}
              </CommandItem>
            ))}
          </CommandGroup>
        )}
        {open &&
          term.length >= 2 &&
          on.map(
            (space) =>
              space.palette && (
                <SpaceResults
                  key={space.id}
                  space={space}
                  palette={space.palette}
                  term={term}
                  go={go}
                />
              ),
          )}
      </CommandList>
      <div className="flex gap-4 border-t px-3 py-2 text-[11px] text-muted-foreground">
        <span>↑↓ {t('shell.palette.hintNavigate')}</span>
        <span>↵ {t('shell.palette.hintOpen')}</span>
      </div>
    </CommandDialog>
  )
}

/** A space's own results, then the row that searches its list page. */
function SpaceResults({
  space,
  palette,
  term,
  go,
}: {
  space: Space
  palette: NonNullable<Space['palette']>
  term: string
  go: (to: string) => void
}) {
  const { t } = useI18n()
  const { Results } = palette
  return (
    <CommandGroup heading={t(palette.heading)}>
      <Suspense fallback={null}>
        <Results term={term} open={go} />
      </Suspense>
      <CommandItem
        value={`search:${space.id}`}
        onSelect={() => go(spaceSearchPath(space, term))}
      >
        <Search />
        <span className="truncate">
          {t('spaces.palette.search', { space: t(space.name), query: term })}
        </span>
      </CommandItem>
    </CommandGroup>
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
