/** Shapes shared by the History screen's lists and its detail panel. */

/** One visit row, whether it came from the timeline, full-text or semantic search. */
export interface VisitItem {
  id: number
  profileId: string
  url: string
  title: string | null
  domain: string
  /** Epoch milliseconds. */
  visitTime: number
  /** 0–1 relevance, semantic results only. */
  score?: number
  /**
   * Full-text and regex results list each page once: the row is the page's
   * newest matching visit and this is how many visits matched.
   */
  visitCount?: number
}

/** What the detail panel shows: a visit, or a starred page that has no visit row. */
export interface DetailTarget {
  url: string
  title: string | null
  domain: string
  visitId?: number
  profileId?: string
}

export interface HistoryFilters {
  startTimeMs: number | null
  endTimeMs: number | null
  browserKind: string | null
  domain: string | null
}

export function targetOf(item: VisitItem): DetailTarget {
  return {
    url: item.url,
    title: item.title,
    domain: item.domain,
    visitId: item.id,
    profileId: item.profileId,
  }
}

/** Profile ids look like `chrome:Default`; the part before the colon is the browser kind. */
export function browserKindOf(profileId: string) {
  return profileId.split(':')[0]
}
