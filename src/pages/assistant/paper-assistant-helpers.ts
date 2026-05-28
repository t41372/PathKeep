/**
 * Pure helpers shared by `paper-assistant-panel.tsx`.
 *
 * Lives in its own file because the React fast-refresh rule wants the
 * component file to export only components — moving the citation
 * adapter here keeps the panel file fast-refresh-friendly without
 * losing test coverage.
 */

import type { PaperAssistantEvidence } from '@/components/explorer-paper'

/**
 * Project AiAssistantCitation-shaped rows onto the paper Evidence shape.
 * Domain is derived from the URL with the `www.` prefix stripped; an
 * unparseable URL falls back to an empty domain.
 */
export function citationsToEvidence(
  citations: readonly {
    url: string
    title?: string | null
    visitedAt: string
  }[],
): PaperAssistantEvidence[] {
  return citations.map((citation, index) => {
    let domain = ''
    try {
      domain = new URL(citation.url).hostname.replace(/^www\./, '')
    } catch {
      domain = ''
    }
    return {
      id: `${citation.url}-${index}`,
      date: citation.visitedAt.slice(0, 10),
      title: citation.title?.trim() ? citation.title : citation.url,
      domain,
      url: citation.url,
    }
  })
}
