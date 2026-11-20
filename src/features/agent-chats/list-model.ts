/**
 * The list's pages as the rows the virtual list draws: day or project
 * headings, conversations, and a project's "more" row. Every kind has a
 * fixed height, so each row's place is known without measuring, the
 * pinned heading can be worked out from the scroll offset alone, and
 * appending a page never moves what is on screen.
 *
 * Responsibilities: flattening pages in order, a day's heading once even
 * when its conversations span pages (its count is the backend's count of
 * the whole day), and each row's offset. Not responsible for: drawing.
 */
import type {
  ChatDayGroup,
  ChatGroup,
  ChatListPage,
  ChatProject,
  ChatProjectGroup,
  ChatSummary,
} from '@/lib/backend-client/agent-chats'

/** A heading: free space above it, then a band. Pinned, only the band shows. */
export const BAND_HEIGHT = 30
export const HEADER_HEIGHT = BAND_HEIGHT + 12
export const ROW_HEIGHT = 60
export const MORE_HEIGHT = 30

export type ListItem =
  | {
      kind: 'header'
      key: string
      /** A day group (`YYYY-MM-DD`, `null` undated) or a project group. */
      day?: string | null
      project?: ChatProject | null
      count: number
    }
  | { kind: 'chat'; key: string; chat: ChatSummary }
  | { kind: 'more'; key: string; project: ChatProject; remaining: number }

export interface ListModel {
  items: ListItem[]
  /** Each item's top, in pixels; one more entry for the total height. */
  starts: number[]
  /** Indexes of the headings, in order. */
  headers: number[]
  /** The conversations in order, for ↑/↓. */
  chats: ChatSummary[]
}

export function itemHeight(item: ListItem) {
  if (item.kind === 'header') return HEADER_HEIGHT
  if (item.kind === 'more') return MORE_HEIGHT
  return ROW_HEIGHT
}

function dayItems(pages: ChatListPage[]): ListItem[] {
  const counts = new Map<string | null, number>()
  for (const page of pages)
    for (const group of page.groups as ChatDayGroup[])
      if (!counts.has(group.day)) counts.set(group.day, group.count)
  const items: ListItem[] = []
  let current: string | null | undefined
  for (const page of pages) {
    for (const chat of page.items) {
      const day = chat.day ?? null
      if (day !== current) {
        current = day
        items.push({
          kind: 'header',
          key: `d:${day ?? 'none'}`,
          day,
          count: counts.get(day) ?? 0,
        })
      }
      items.push({ kind: 'chat', key: `c:${chat.id}`, chat })
    }
  }
  return items
}

function projectItems(pages: ChatListPage[]): ListItem[] {
  const items: ListItem[] = []
  for (const page of pages) {
    for (const group of page.groups as ChatProjectGroup[]) {
      const key = group.project?.id ?? ''
      items.push({
        kind: 'header',
        key: `p:${key}`,
        project: group.project,
        count: group.count,
      })
      for (const chat of group.items)
        items.push({ kind: 'chat', key: `c:${chat.id}`, chat })
      if (group.remaining > 0 && group.project)
        items.push({
          kind: 'more',
          key: `m:${key}`,
          project: group.project,
          remaining: group.remaining,
        })
    }
  }
  return items
}

export function buildList(pages: ChatListPage[], group: ChatGroup): ListModel {
  const items =
    group === 'project'
      ? projectItems(pages)
      : group === 'day'
        ? dayItems(pages)
        : pages.flatMap((page) =>
            page.items.map((chat): ListItem => ({
              kind: 'chat',
              key: `c:${chat.id}`,
              chat,
            })),
          )
  const starts = new Array<number>(items.length + 1)
  const headers: number[] = []
  const chats: ChatSummary[] = []
  let top = 0
  items.forEach((item, index) => {
    starts[index] = top
    top += itemHeight(item)
    if (item.kind === 'header') headers.push(index)
    if (item.kind === 'chat') chats.push(item.chat)
  })
  starts[items.length] = top
  return { items, starts, headers, chats }
}

/**
 * The heading pinned at the top for a scroll offset, and how far the next
 * heading has pushed it up (≤ 0). None while a group's own heading is
 * still in its place, so only one ever shows at the top.
 */
export function pinnedHeading(model: ListModel, offset: number) {
  const bandTop = (h: number) =>
    model.starts[model.headers[h]] + (HEADER_HEIGHT - BAND_HEIGHT) - offset
  let low = 0
  let high = model.headers.length - 1
  let found = -1
  while (low <= high) {
    const middle = (low + high) >> 1
    if (bandTop(middle) < 0) {
      found = middle
      low = middle + 1
    } else high = middle - 1
  }
  if (found < 0) return { index: -1, shift: 0 }
  const next =
    found + 1 < model.headers.length
      ? bandTop(found + 1)
      : Number.POSITIVE_INFINITY
  return {
    index: model.headers[found],
    shift: Math.min(0, Math.round(next - BAND_HEIGHT)),
  }
}
