/**
 * Whether Records should send a search by meaning, from what the index's
 * status says about the index itself: no embedding model chosen, a first
 * build running or paused, the last build failed, or none built. Whether
 * AI, semantic search, this computer's switch or Agent conversations are
 * off is left to the search: the status answers `enabled: false` alike for
 * all of them, and the search's refusal names which
 * (`use-semantic-search.ts`).
 *
 * Responsibilities: turning the status, or a refused read of it, into one
 * state, and a short signature of the status that a refused search waits
 * to see change. Not responsible for: keeping the status current
 * (`useAgentSemanticSync` reads it again when the AI settings, the switches
 * or the build change), any copy, or running the search.
 */
import type {
  SemanticGeneration,
  SemanticIndexStatus,
  SemanticJob,
} from '@/lib/backend-client/agent-semantic'
import {
  aiRefusalCode,
  type AiRefusalCode,
} from '@/lib/backend-client/ai-refusals'
import { useAgentKind } from '@/lib/queries/agent-backup'
import {
  isActiveJob,
  useAgentSemanticIndex,
} from '@/lib/queries/agent-semantic'

export type SemanticGate =
  | { kind: 'loading' }
  /** The status could not be read. */
  | { kind: 'unknown'; error: Error }
  /** Refused before searching: the status read was, or it names a missing model. */
  | { kind: 'refused'; code: AiRefusalCode }
  /** Nothing published yet for the chosen model; a build is running or paused. */
  | { kind: 'building'; job: SemanticJob }
  /** Nothing published, and the last build failed, for a typed reason or not. */
  | { kind: 'failed'; code: AiRefusalCode | null }
  | { kind: 'notBuilt' }
  | {
      kind: 'search'
      /** The generation searches use; `null` when the status doesn't say. */
      serving: SemanticGeneration | null
      /** Conversations changed since it was built. */
      stale: boolean
      /** The build that brings it up to date, while one runs. */
      job: SemanticJob | null
    }

/** Search, and let the backend say whether it may. */
const ask: SemanticGate = {
  kind: 'search',
  serving: null,
  stale: false,
  job: null,
}

function fromStatus(status: SemanticIndexStatus): SemanticGate {
  if (!status.enabled) return ask
  const reason = status.reason ? aiRefusalCode(status.reason) : null
  if (reason) return { kind: 'refused', code: reason }
  const { serving, job } = status
  if (serving)
    return {
      kind: 'search',
      serving,
      stale: status.stale ?? false,
      job: isActiveJob(job) ? job : null,
    }
  // A build that is stopping throws its part away: there will be nothing.
  if (job && isActiveJob(job) && job.state !== 'cancelling')
    return { kind: 'building', job }
  if (job?.state === 'failed')
    return { kind: 'failed', code: job.error ? aiRefusalCode(job.error) : null }
  return { kind: 'notBuilt' }
}

/** What a refused search waits to see change before it asks again. */
function signatureOf(status: SemanticIndexStatus) {
  return [
    status.enabled,
    status.reason ?? '',
    status.serving?.generation ?? '',
  ].join(':')
}

export function useSemanticGate(): { gate: SemanticGate; signature: string } {
  const { row } = useAgentKind()
  const status = useAgentSemanticIndex()
  // With Agent conversations off the status isn't read; the search says so.
  if (row?.state === 'off') return { gate: ask, signature: 'kind-off' }
  if (status.data)
    return {
      gate: fromStatus(status.data),
      signature: signatureOf(status.data),
    }
  if (status.isError && !status.isFetching) {
    const code = aiRefusalCode(status.error)
    return code
      ? { gate: { kind: 'refused', code }, signature: code }
      : { gate: { kind: 'unknown', error: status.error }, signature: 'unknown' }
  }
  return { gate: { kind: 'loading' }, signature: 'loading' }
}
