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

/**
 * The address prefixes outside a page's own path that light it in the
 * rail: each of its `elsewhere` screens up to its first parameter
 * (`chat/:id` → `/agent/chat/`).
 */
export function pageElsewhere(space: Space, page: SpacePage): string[] {
  return (page.elsewhere ?? []).map(
    (other) => `/${space.id}/${other.path.split(':')[0]}`,
  )
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
 * One Agent conversation in the full-page reader (`#/agent/chat/:id`, an
 * `elsewhere` screen of the space's list page).
 */
export function agentChatPath(id: string) {
  return `/${'agent' satisfies SpaceWithPagesId}/chat/${encodeURIComponent(id)}`
}

/**
 * Records with one conversation open at one of its records (`?chat=…&at=…`,
 * the ordinal first so the source id can hold anything): where a semantic
 * search result, an Ask citation or a `pathkeep://conversation/open` link
 * leads.
 */
export function agentRecordPath(
  id: string,
  at: { sourceId: string; ordinal: number },
) {
  const search = new URLSearchParams({
    chat: id,
    at: `${at.ordinal}:${at.sourceId}`,
  })
  return `/${'agent' satisfies SpaceWithPagesId}/chats?${search.toString()}`
}

/**
 * The record a `pathkeep://conversation/open?conversationId=…&sourceId=…&ordinal=…`
 * link names (what Ask's model is told to cite, and what MCP returns), or
 * null for any other link.
 */
export function conversationLinkTarget(href: string) {
  let link: URL
  try {
    link = new URL(href)
  } catch {
    return null
  }
  if (link.protocol !== 'pathkeep:' || link.host !== 'conversation') return null
  const conversationId = link.searchParams.get('conversationId')
  const sourceId = link.searchParams.get('sourceId')
  const ordinal = Number(link.searchParams.get('ordinal'))
  return conversationId && sourceId && Number.isInteger(ordinal) && ordinal >= 0
    ? { conversationId, sourceId, ordinal }
    : null
}

/**
 * The Agent space's Insights (`#/agent/insights`); with a project, only
 * that project's conversations (`?project=<id>`).
 */
export function agentInsightsPath(projectId?: string) {
  const search = projectId
    ? `?${new URLSearchParams({ project: projectId }).toString()}`
    : ''
  return `/${'agent' satisfies SpaceWithPagesId}/insights${search}`
}

/** The Agent space's projects (`#/agent/projects`). */
export function agentProjectsPath(search = '') {
  return `/${'agent' satisfies SpaceWithPagesId}/projects${search}`
}

/** One project's page (`#/agent/projects/:id`): its conversations. */
export function agentProjectPath(id: string) {
  return `${agentProjectsPath()}/${encodeURIComponent(id)}`
}

/**
 * The space a path is in, or null for a shared screen (Ask, Settings). A
 * space whose kind is off still owns its paths; they show that it is off.
 */
export function spaceOfPath(pathname: string): SpaceId | null {
  const first = pathname.split('/')[1]
  return spaces.find((space) => space.id === first)?.id ?? null
}
