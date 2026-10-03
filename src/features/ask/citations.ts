/** Helpers for the pages an answer cites. */
import type { AiChatCitation } from '@/lib/types'

export function hostOf(url: string) {
  try {
    return new URL(url).hostname.replace(/^www\./, '')
  } catch {
    return url
  }
}

export function historyLink(citation: AiChatCitation) {
  const query = citation.title?.trim() || hostOf(citation.url)
  return `/history?q=${encodeURIComponent(query)}&visit=${citation.historyId}`
}

/** One chip per page: the same URL cited twice is shown once. */
export function uniqueCitations(citations: readonly AiChatCitation[]) {
  const seen = new Set<string>()
  return citations.filter((citation) => {
    const key = citation.canonicalUrl || citation.url
    if (seen.has(key)) return false
    seen.add(key)
    return true
  })
}
