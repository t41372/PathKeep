/**
 * Whether Records can search by meaning right now, and if not, why: the
 * one thing a person has to do, and where. Read from what the app already
 * knows rather than from a refused search, because the backend refuses
 * three of these with the same code (`kind-unavailable`) and words meant
 * for logs.
 *
 * Responsibilities: combining the AI settings (Settings › AI's semantic
 * search), this computer's "Build a semantic search index" switch
 * (Settings › Agent conversations) and the index's status into one state.
 * Not responsible for: keeping the status current (`useAgentSemanticSync`
 * reads it again when the AI settings, the switch or the build change),
 * any copy, or running the search.
 */
import type {
  SemanticGeneration,
  SemanticIndexStatus,
  SemanticJob,
} from '@/lib/backend-client/agent-semantic'
import { useSnapshot } from '@/lib/queries/app'
import {
  isActiveJob,
  useAgentSemanticIndex,
} from '@/lib/queries/agent-semantic'
import { useViewerPreferences } from '@/lib/queries/agent-viewer'

export type SemanticGate =
  | { kind: 'loading' }
  /** The status could not be read. */
  | { kind: 'unknown'; error: Error }
  /** Semantic search is off in Settings › AI (and maybe the switch too). */
  | { kind: 'aiOff'; switchOff: boolean }
  /** "Build a semantic search index" is off in Settings › Agent conversations. */
  | { kind: 'switchOff' }
  /** Both look on here, yet the backend says no: say both places. */
  | { kind: 'off' }
  | { kind: 'noModel' }
  /** Nothing published yet for the chosen model; a build is running or paused. */
  | { kind: 'building'; job: SemanticJob }
  | { kind: 'failed' }
  | { kind: 'notBuilt' }
  | {
      kind: 'ready'
      serving: SemanticGeneration
      /** Conversations changed since it was built. */
      stale: boolean
      /** The build that brings it up to date, while one runs. */
      job: SemanticJob | null
    }

function fromStatus(status: SemanticIndexStatus): SemanticGate {
  if (!status.enabled) return { kind: 'off' }
  if (status.reason === 'embedding-provider-unavailable')
    return { kind: 'noModel' }
  const { serving, job } = status
  if (serving)
    return {
      kind: 'ready',
      serving,
      stale: status.stale ?? false,
      job: isActiveJob(job) ? job : null,
    }
  // A build that is stopping throws its part away: there will be nothing.
  if (job && isActiveJob(job) && job.state !== 'cancelling')
    return { kind: 'building', job }
  if (job?.state === 'failed') return { kind: 'failed' }
  return { kind: 'notBuilt' }
}

export function useSemanticGate(): SemanticGate {
  const ai = useSnapshot().config.ai
  const aiOn = ai.enabled && ai.semanticIndexEnabled
  const prefs = useViewerPreferences()
  const switchOn = prefs.data?.buildSemanticIndex
  const status = useAgentSemanticIndex()

  if (!aiOn) return { kind: 'aiOff', switchOff: switchOn === false }
  if (switchOn === false) return { kind: 'switchOff' }
  // An "off" from before Settings › AI changed is being read again.
  if (status.data && !(status.isFetching && !status.data.enabled))
    return fromStatus(status.data)
  if (status.isError && !status.isFetching)
    return { kind: 'unknown', error: status.error }
  return { kind: 'loading' }
}
