/** Saved conversations: the list query and day grouping for the sidebar. */
import { useInfiniteQuery } from '@tanstack/react-query'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { startOfDay } from '@/lib/i18n'
import type { AgentConversationSummary } from '@/lib/types'

export const conversationsKey = ['ask', 'conversations'] as const

const PAGE_SIZE = 100

/** Every saved conversation, newest first; more load as the list scrolls. */
export function useConversations() {
  return useInfiniteQuery({
    queryKey: conversationsKey,
    initialPageParam: null as string | null,
    queryFn: ({ pageParam }) =>
      intelligenceClient.listConversations({
        limit: PAGE_SIZE,
        cursor: pageParam,
      }),
    getNextPageParam: (last) => last.nextCursor ?? undefined,
    select: (data) => data.pages.flatMap((page) => page.conversations),
    staleTime: 30_000,
  })
}

export interface ConversationGroup {
  day: Date
  items: AgentConversationSummary[]
}

export function groupByDay(
  items: readonly AgentConversationSummary[],
): ConversationGroup[] {
  const groups: ConversationGroup[] = []
  for (const item of items) {
    const day = startOfDay(item.updatedAt)
    const last = groups[groups.length - 1]
    if (last && last.day.getTime() === day.getTime()) last.items.push(item)
    else groups.push({ day, items: [item] })
  }
  return groups
}
