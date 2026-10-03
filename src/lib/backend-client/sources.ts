/**
 * Per-source visit counts. Browser discovery (in the snapshot) only knows
 * file sizes; how many visits each source contributed comes from the archive.
 */
import { call } from './shared'

export interface SourceStats {
  /** Matches `BrowserProfile.profileId` and `config.selectedProfileIds`. */
  profileId: string
  browserName: string
  profileName: string
  visitCount: number
  firstVisitAt: string | null
  lastVisitAt: string | null
}

export const sourcesClient = {
  stats: () => call<SourceStats[]>('load_source_stats'),
}
