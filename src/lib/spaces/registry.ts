/**
 * The kinds of data PathKeep keeps, each as a space: its own pages in the
 * rail, its own routes under `#/<space>/…`, its own entries in ⌘K, its own
 * backup page and its own hue (`docs/design/data-spaces-tradeoff.md` §4).
 *
 * Responsibilities: the one list the shell is drawn from. The switcher's
 * order and shortcut numbers, the rail's pages, the routes, ⌘K's "Go to" and
 * "Search in" rows, a space's status line and its colour all come from here,
 * so a new kind of data is one entry plus its own pages; nothing else in the
 * shell names a space. Which spaces are on.
 * Not responsible for: building paths (`paths.ts`), remembering the last
 * space (`last-space.ts`), or drawing anything.
 *
 * Screens and the status line are `import()` thunks, so this list loads no
 * screen code and screens can import the path builder without a cycle.
 */
import {
  ChartColumn,
  Globe,
  HardDriveDownload,
  History,
  House,
  type LucideIcon,
} from 'lucide-react'
import { lazy, type ComponentType } from 'react'
import type { MessageKey } from '@/lib/i18n'

type Screen = () => Promise<{ default: ComponentType }>

/** What a space shows in ⌘K once two or more characters are typed. */
export interface PaletteResultsProps {
  /** The debounced words, at least two characters. */
  term: string
  /** Closes the palette and goes to `to`. */
  open: (to: string) => void
}

export interface SpacePage {
  /** The page's part of the URL: `#/<space>/<id>`. */
  id: string
  label: MessageKey
  icon: LucideIcon
  screen: Screen
  /**
   * Screens under this page (`#/<space>/<id>/<path>`), which light this
   * page in the rail.
   */
  children?: readonly { path: string; screen: Screen }[]
}

export interface Space {
  /** The space's part of the URL, and its `data-space` on `<html>`. */
  id: string
  /** The full name: the switcher's menu, ⌘K, headings. */
  name: MessageKey
  /** The name on the switcher in the rail, which has room for about 64 px. */
  shortName: MessageKey
  icon: LucideIcon
  /**
   * The space's oklch hue: its tile in the switcher, and, unless
   * `baseTokens`, the window tint and accent family while it is current
   * (`--space-h`, `src/index.css`).
   */
  hue: number
  /**
   * Keeps PathKeep's own colour tokens instead of deriving them from `hue`.
   * Browsing does, so it looks exactly as it did before spaces.
   */
  baseTokens?: boolean
  /** In rail order. The first is where the space opens. */
  pages: readonly SpacePage[]
  /** The page that lists the records and takes `?q=` (⌘K's "Search in"). */
  listPage: string
  /** The page that backs up this kind of data. */
  backupPage: string
  /** One line for the switcher's menu, loaded when the menu opens. */
  Status: ComponentType
  /** ⌘K's results group for this space. */
  palette: {
    heading: MessageKey
    Results: ComponentType<PaletteResultsProps>
  }
}

const browsing = {
  id: 'browsing',
  name: 'spaces.browsing.name',
  shortName: 'spaces.browsing.shortName',
  icon: Globe,
  hue: 48,
  baseTokens: true,
  pages: [
    {
      id: 'home',
      label: 'shell.nav.home',
      icon: House,
      screen: () => import('@/features/home/home-page'),
    },
    {
      id: 'history',
      label: 'shell.nav.history',
      icon: History,
      screen: () => import('@/features/history/history-page'),
    },
    {
      id: 'insights',
      label: 'shell.nav.insights',
      icon: ChartColumn,
      screen: () => import('@/features/insights/insights-page'),
      children: [
        {
          path: 'day/:date',
          screen: () => import('@/features/insights/day-page'),
        },
        {
          path: 'site/:domain',
          screen: () => import('@/features/insights/site-page'),
        },
        {
          path: 'search/:query',
          screen: () => import('@/features/insights/search-page'),
        },
        {
          path: 'page/:url',
          screen: () => import('@/features/insights/page-page'),
        },
      ],
    },
    {
      id: 'backup',
      label: 'shell.nav.backup',
      icon: HardDriveDownload,
      screen: () => import('@/features/backup/backup-page'),
    },
  ],
  listPage: 'history',
  backupPage: 'backup',
  Status: lazy(() => import('@/features/home/space-status')),
  palette: {
    heading: 'shell.palette.pageResults',
    Results: lazy(() => import('@/features/history/palette-results')),
  },
} as const satisfies Space

/** Every space PathKeep can keep, in the switcher's order. */
export const spaces = [browsing] as const satisfies readonly Space[]

type Registered = (typeof spaces)[number]
export type SpaceId = Registered['id']
/** The page ids of one space, so a path to a page it lacks does not compile. */
export type PageOf<S extends SpaceId> = Extract<
  Registered,
  { id: S }
>['pages'][number]['id']

/**
 * The spaces that are on, in the switcher's order: the rail, the menu, ⌘K
 * and ⌘1…⌘9 show these. Every registered space is on until the user can
 * turn kinds of data on and off.
 */
export function spacesOn(): readonly Space[] {
  return spaces
}

/**
 * Registered spaces that are not on. The switcher's menu offers to keep
 * another kind of data only while this is not empty.
 */
export function spacesOff(): readonly Space[] {
  const on = new Set(spacesOn().map((space) => space.id))
  return spaces.filter((space) => !on.has(space.id))
}

export function spaceById(id: string): Space | undefined {
  return spaces.find((space) => space.id === id)
}
