/**
 * The kinds of data PathKeep keeps, each as a space: its own pages in the
 * rail, its own routes under `#/<space>/…`, its own entries in ⌘K, its own
 * backup page and its own hue (`docs/design/data-spaces-tradeoff.md` §4).
 *
 * Responsibilities: the one list the shell is drawn from. The switcher's
 * order and shortcut numbers, the rail's pages, the routes, ⌘K's "Go to" and
 * "Search in" rows, a space's status line and its colour all come from here,
 * so a new kind of data is one entry plus its own pages; nothing else in the
 * shell names a space. Which spaces are on, from the backend's answer
 * (`kind-states.ts`).
 * Not responsible for: building paths (`paths.ts`), remembering the last
 * space (`last-space.ts`), or drawing anything.
 *
 * Screens and the status line are `import()` thunks, so this list loads no
 * screen code and screens can import the path builder without a cycle.
 */
import {
  Bot,
  ChartColumn,
  Globe,
  HardDriveDownload,
  History,
  House,
  type LucideIcon,
} from 'lucide-react'
import { lazy, type ComponentType, type ReactNode } from 'react'
import type { DataKindId, KindState } from '@/lib/backend-client/data-kinds'
import type { MessageKey } from '@/lib/i18n'
import {
  kindStateOf,
  readKindList,
  useKindList,
  type KindListAnswer,
} from './kind-states'

type Screen = () => Promise<{ default: ComponentType }>

/**
 * What a space shows in ⌘K once two or more characters are typed: its own
 * group, under `heading`, with its rows and then `searchRow`. A group with
 * no rows and no search row is not drawn at all (an empty heading would be
 * left otherwise: the palette does its own matching).
 */
export interface PaletteResultsProps {
  /** The debounced words, at least two characters. */
  term: string
  /** Closes the palette and goes to `to`. */
  open: (to: string) => void
  heading: string
  /** "Search … for …", or `null` for a space whose list cannot search. */
  searchRow: ReactNode
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

/**
 * A kind of data PathKeep knows, whether or not this build has its pages
 * yet. Enough for the switcher's "keep another kind", Settings › Manage
 * data types and the page an address of a kind that is off shows.
 */
export interface DataSpace {
  /** The space's part of the URL, and its `data-space` on `<html>`. */
  id: string
  /** The backend's id for this kind of data (`list_data_kinds`). */
  kind: DataKindId
  /**
   * Whether the kind is on in an archive that never chose, as the backend
   * assumes. Until the backend answers, the shell shows the spaces that are
   * on by default, so the rail is never empty and never flashes.
   */
  onByDefault: boolean
  /** The full name: the switcher's menu, ⌘K, headings. */
  name: MessageKey
  /** The name on the switcher in the rail, which has room for about 64 px. */
  shortName: MessageKey
  /** One line on what is lost without PathKeep. */
  why: MessageKey
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
}

/** A kind of data with its pages: what the rail, the switcher, ⌘K and the routes show. */
export interface Space extends DataSpace {
  /** In rail order. The first is where the space opens. */
  pages: readonly SpacePage[]
  /** The page that lists the records and takes `?q=` (⌘K's "Search in"). */
  listPage: string
  /** The page that backs up this kind of data. */
  backupPage: string
  /** One line for the switcher's menu, loaded when the menu opens. */
  Status: ComponentType
  /**
   * ⌘K's results group for this space, and its "Search in" row unless
   * `searchRow` is false (a space whose list page cannot search yet).
   */
  palette?: {
    heading: MessageKey
    Results: ComponentType<PaletteResultsProps>
    searchRow?: boolean
  }
}

const browsing = {
  id: 'browsing',
  kind: 'browsing',
  onByDefault: true,
  name: 'spaces.browsing.name',
  shortName: 'spaces.browsing.shortName',
  why: 'dataKinds.why.browsing',
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

/**
 * Agent conversations. Its pages are insights, records, projects and
 * backup (`data-spaces-tradeoff.md` §4.1); this build has the backup page,
 * so for now it is also where the space opens and its list page. ⌘K finds
 * its sources and recent backups, with no "Search in" row (the backup page
 * does not search). Insights, records and projects come with the
 * conversation read model: insights goes first (where the space opens),
 * records becomes `listPage`, and ⌘K finds conversations and searches them.
 */
const agent = {
  id: 'agent',
  kind: 'agent-history',
  onByDefault: false,
  name: 'spaces.agent.name',
  shortName: 'spaces.agent.shortName',
  why: 'dataKinds.why.agent',
  icon: Bot,
  hue: 195,
  pages: [
    {
      id: 'backup',
      label: 'shell.nav.backup',
      icon: HardDriveDownload,
      screen: () => import('@/features/agent-backup/agent-backup-page'),
    },
  ],
  listPage: 'backup',
  backupPage: 'backup',
  Status: lazy(() => import('@/features/agent-backup/agent-status')),
  palette: {
    heading: 'spaces.agent.name',
    Results: lazy(() => import('@/features/agent-backup/palette-results')),
    searchRow: false,
  },
} as const satisfies Space

/** Every kind of data PathKeep can keep, in the switcher's order. */
export const spaces = [browsing, agent] as const satisfies readonly DataSpace[]

type Registered = (typeof spaces)[number]
export type SpaceId = Registered['id']
/** The spaces this build has pages for. */
type WithPages = Extract<Registered, { pages: unknown }>
export type SpaceWithPagesId = WithPages['id']
/** The page ids of one space, so a path to a page it lacks does not compile. */
export type PageOf<S extends SpaceWithPagesId> = Extract<
  WithPages,
  { id: S }
>['pages'][number]['id']

export function hasPages(space: DataSpace): space is Space {
  return 'pages' in space && (space as Space).pages.length > 0
}

/** On or paused: the kind's pages are in the rail. Before the answer, its default. */
function shown(space: DataSpace, list: KindListAnswer) {
  const state = kindStateOf(list, space.kind)
  return state === undefined ? space.onByDefault : state !== 'off'
}

let memo: { list: KindListAnswer; on: readonly Space[] } | null = null

/** The spaces on in `list`, the same array for the same answer. */
function spacesOnIn(list: KindListAnswer): readonly Space[] {
  if (memo && memo.list === list) return memo.on
  const withPages = (spaces as readonly DataSpace[]).filter(hasPages)
  const shownNow = withPages.filter((space) => shown(space, list))
  const on = shownNow.length > 0 ? shownNow : withPages.slice(0, 1)
  memo = { list, on }
  return on
}

/**
 * The spaces that are on or paused, in the switcher's order: the rail, the
 * menu, ⌘K and ⌘1…⌘9 show these. Before the backend answers (and after a
 * lock clears its answer), the spaces on by default. Never empty: should no
 * kind with pages be on, the first space with pages stays.
 */
export function spacesOn(): readonly Space[] {
  return spacesOnIn(readKindList())
}

/** `spacesOn()` that re-renders when the backend's answer changes. */
export function useSpacesOn(): readonly Space[] {
  return spacesOnIn(useKindList())
}

function spacesOffIn(list: KindListAnswer): DataSpace[] {
  return spaces.filter((space) => {
    const state: KindState | undefined = kindStateOf(list, space.kind)
    return state === undefined ? !space.onByDefault : state === 'off'
  })
}

/**
 * Registered kinds that are off. The switcher's menu offers to keep another
 * kind of data only while this is not empty.
 */
export function spacesOff(): DataSpace[] {
  return spacesOffIn(readKindList())
}

/** `spacesOff()` that re-renders when the backend's answer changes. */
export function useSpacesOff(): DataSpace[] {
  return spacesOffIn(useKindList())
}

export function spaceById(id: string): DataSpace | undefined {
  return spaces.find((space) => space.id === id)
}

/** The space of one of the backend's kinds of data. */
export function spaceOfKind(kind: DataKindId): DataSpace | undefined {
  return spaces.find((space) => space.kind === kind)
}
