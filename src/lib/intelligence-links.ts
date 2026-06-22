/**
 * Shared evidence and assistant-link helpers for intelligence-adjacent routes.
 *
 * Why this file exists:
 * - M11 moves link-building and citation dedupe out of the mixed intelligence
 *   barrel so route grammar and evidence plumbing can evolve independently.
 */

import type { AiAssistantCitation } from './types'
import type { DateRange } from './core-intelligence'

export function evidenceHref(evidence: {
  profileId?: string | null
  url?: string | null
  domain?: string | null
  title?: string | null
  dateRange?: DateRange | null
}) {
  const params = new URLSearchParams()
  if (evidence.profileId) params.set('profileId', evidence.profileId)
  if (evidence.domain) params.set('domain', evidence.domain)
  if (evidence.dateRange) {
    params.set('start', evidence.dateRange.start)
    params.set('end', evidence.dateRange.end)
  }
  if (evidence.url) params.set('q', evidence.url)
  else if (evidence.title) params.set('q', evidence.title)
  const query = params.toString()
  return query ? `/explorer?${query}` : '/explorer'
}

/**
 * Builds an in-app link to the assistant, prefilling only the question.
 *
 * No profile scope is appended on purpose: the live agent chat path (`ai_chat_send`) searches the
 * WHOLE archive — its `AiChatSendRequest` carries no profile filter and the worker builds the agent
 * tool context with `default_profile_id: None`. Advertising a `?profileId=` here would be a scope
 * the backend silently drops (dishonest), so this helper deliberately takes only the question. If
 * per-profile scoping is plumbed end-to-end later, re-add the param AND read it in the route.
 */
export function assistantHref(question: string) {
  const params = new URLSearchParams()
  params.set('question', question)
  return `/assistant?${params.toString()}`
}

export function dedupeEvidence<
  T extends Pick<AiAssistantCitation, 'historyId' | 'url'>,
>(items: T[]) {
  const seen = new Set<string>()
  return items.filter((item) => {
    const key = `${item.historyId}:${item.url}`
    if (seen.has(key)) return false
    seen.add(key)
    return true
  })
}
