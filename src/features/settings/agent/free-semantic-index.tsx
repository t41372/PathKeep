/**
 * The Agent conversations' semantic index while it isn't used (the switch
 * off, or semantic search off in Settings › AI): its size and Free…, through
 * `preview_free_agent_semantic_index` and `free_agent_semantic_index`. The
 * line and the confirmation are `KeptIndex`; this file gives it the Agent
 * index's commands and words.
 *
 * Not responsible for: whether the index is kept (the status in
 * `semantic-status.tsx`) or building it again (turning the switch on).
 */
import { agentSemanticClient } from '@/lib/backend-client/agent-semantic'
import { refetchSemanticIndex } from '@/lib/queries/agent-semantic'
import { KeptIndex, type KeptIndexSource } from '../kept-index'

const AGENT_INDEX: KeptIndexSource = {
  name: 'agent-semantic',
  previewFree: agentSemanticClient.previewFree,
  free: agentSemanticClient.free,
  onFreed: refetchSemanticIndex,
  refused: {
    // A build that was running when the switch went off is still stopping.
    'archive-busy': 'settingsAgent.ai.index.kept.busy',
    // The switch, or semantic search, is on again: the index is in use.
    'task-action-unavailable': 'settingsAgent.ai.index.kept.inUse',
  },
  words: {
    line: 'settingsAgent.ai.index.kept.line',
    free: 'settingsAgent.ai.index.kept.free',
    freeLabel: 'settingsAgent.ai.index.kept.freeLabel',
    title: 'settingsAgent.ai.index.kept.title',
    reading: 'settingsAgent.ai.index.kept.reading',
    body: 'settingsAgent.ai.index.kept.body',
    nothing: 'settingsAgent.ai.index.kept.nothing',
    changed: 'settingsAgent.ai.index.kept.changed',
    confirm: 'settingsAgent.ai.index.kept.confirm',
    freeing: 'settingsAgent.ai.index.kept.freeing',
    done: 'settingsAgent.ai.index.kept.done',
    readFailed: 'settingsAgent.ai.index.kept.readFailed',
    failed: 'settingsAgent.ai.index.kept.failed',
  },
}

/** The kept index's size and Free…; nothing when none is kept (`keptBytes` null or "0"). */
export function KeptSemanticIndex({ keptBytes }: { keptBytes: string | null }) {
  return (
    <KeptIndex
      source={AGENT_INDEX}
      keptBytes={keptBytes}
      innerClassName="pb-3"
    />
  )
}
