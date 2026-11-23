/**
 * Browsing history's semantic search index under the semantic search switch
 * while semantic search is off: what it takes on disk and Free…, through
 * `KeptIndex` with the browsing index's commands (`semantic-index-free.ts`).
 */
import { KeptIndex } from './kept-index'
import { BROWSING_INDEX } from './semantic-index-free'

/**
 * `bytes` is `semanticSidecarBytes`, every model's index files; nothing shows
 * at 0. It sits in the row's `gap-3` column, so the spacing above it is its
 * own and slides with it.
 */
export function KeptBrowsingIndex({ bytes }: { bytes: number }) {
  return (
    <KeptIndex
      source={BROWSING_INDEX}
      keptBytes={String(bytes)}
      className="-mt-3"
      innerClassName="pt-3"
    />
  )
}
