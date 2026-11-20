/**
 * What an answer cites and where each citation opens.
 *
 * Responsibilities: telling a history page from a record of an Agent
 * conversation, the path each one opens (History at the visit, the Agent
 * reader at the record), and one chip per thing cited. Not responsible for
 * how a chip looks (`sources.tsx`).
 */
import { agentRecordPath, spacePath } from '@/lib/spaces'
import type { AiChatCitation, AiCitationTarget } from '@/lib/types'

export function hostOf(url: string) {
  try {
    return new URL(url).hostname.replace(/^www\./, '')
  } catch {
    return url
  }
}

/** The Agent conversation record a citation points at; null for a history page. */
export function recordOf(citation: AiChatCitation): AiCitationTarget | null {
  return citation.target?.kind === 'conversation' ? citation.target : null
}

function historyPath(citation: AiChatCitation) {
  const query = citation.title?.trim() || hostOf(citation.url)
  return spacePath(
    'browsing',
    'history',
    `?q=${encodeURIComponent(query)}&visit=${citation.historyId}`,
  )
}

/**
 * Where a citation opens: a record in the Agent reader, scrolled to it; a
 * page in History at the visit cited. A record's `historyId` is negative and
 * its address is a `pathkeep:` link, so it must never go to History.
 */
export function citationPath(citation: AiChatCitation) {
  const record = recordOf(citation)
  return record
    ? agentRecordPath(record.conversationId, record)
    : historyPath(citation)
}

/**
 * What makes two citations the same thing: the conversation and record for
 * an Agent conversation, the page's address otherwise. Newlines keep the two
 * apart, since no address holds one.
 */
export function citationKey(citation: AiChatCitation) {
  const record = recordOf(citation)
  return record
    ? `\nconversation\n${record.conversationId}\n${record.ordinal}\n${record.sourceId}`
    : citation.canonicalUrl || citation.url
}

/** One chip per thing cited: the same page or record cited twice is shown once. */
export function uniqueCitations(citations: readonly AiChatCitation[]) {
  const seen = new Set<string>()
  return citations.filter((citation) => {
    const key = citationKey(citation)
    if (seen.has(key)) return false
    seen.add(key)
    return true
  })
}
