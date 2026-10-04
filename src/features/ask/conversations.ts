/** Saved conversations: the list query and day grouping for the sidebar. */
import { useQuery } from '@tanstack/react-query'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { startOfDay } from '@/lib/i18n'
import type { AgentConversationSummary } from '@/lib/types'

export const conversationsKey = ['ask', 'conversations'] as const

// Newest first, bounded: the sidebar never needs the whole history.
const LIST_LIMIT = 200

export function useConversations() {
  return useQuery({
    queryKey: conversationsKey,
    queryFn: async () =>
      (await intelligenceClient.listConversations({ limit: LIST_LIMIT }))
        .conversations,
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
