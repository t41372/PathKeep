/**
 * Paths into a space's pages, `#/<space>/<page>…`, built from the registry.
 *
 * Responsibilities: every link, redirect and `navigate()` to a space's page
 * goes through here, so no screen holds a literal space path and the routes
 * (`src/app/router.tsx`, drawn from the same registry) cannot drift from the
 * links. Which space a path belongs to.
 * Not responsible for: shared screens (`/ask`, `/settings/…`), which carry
 * no space, or what a page's search parameters mean (each screen's own
 * link helpers, such as `features/insights/links.ts`).
 */
import {
  spaces,
  type PageOf,
  type Space,
  type SpaceId,
  type SpacePage,
  type SpaceWithPagesId,
} from './registry'

/**
 * A page of a space. `rest` follows the page: a deeper path
 * (`/day/2026-01-02`) and/or a query (`?q=…`).
 */
export function spacePath<S extends SpaceWithPagesId>(
  space: S,
  page: PageOf<S>,
  rest = '',
) {
  return `/${space}/${page}${rest}`
}

/** One of a space's pages, for code that walks the registry. */
export function spacePagePath(space: Space, page: SpacePage) {
  return `/${space.id}/${page.id}`
}

/** Where a space opens: its first page. */
export function spaceStartPath(space: Space) {
  return spacePagePath(space, space.pages[0])
}

/** A space's list page, searching for `query` when there is one. */
export function spaceSearchPath(space: Space, query?: string) {
  const search = query ? `?q=${encodeURIComponent(query)}` : ''
  return `/${space.id}/${space.listPage}${search}`
}

/** A space's backup page; `rest` as in `spacePath`. */
export function spaceBackupPath(id: SpaceWithPagesId, rest = '') {
  const space = spaces.find((item) => item.id === id) as Space
  return `/${id}/${space.backupPage}${rest}`
}

/**
 * The space a path is in, or null for a shared screen (Ask, Settings). A
 * space whose kind is off still owns its paths; they show that it is off.
 */
export function spaceOfPath(pathname: string): SpaceId | null {
  const first = pathname.split('/')[1]
  return spaces.find((space) => space.id === first)?.id ?? null
}
