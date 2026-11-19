/**
 * What `detect_data_sources` found, as the screens need it: which Agent
 * tools have a folder on this computer, and which browsers. Found means the
 * folder is there; a folder the system will not let PathKeep read still
 * counts (the preview says why it cannot be read).
 *
 * Not responsible for: asking (`useDetectedSources`) or wording.
 */
import type {
  AgentToolId,
  AgentToolPresence,
  DetectedSources,
} from '@/lib/backend-client/data-kinds'
import { inToolOrder } from './tools'

function agentRows(detected: DetectedSources): AgentToolPresence[] {
  return detected.sources.filter(
    (source): source is AgentToolPresence => source.kind === 'agent-history',
  )
}

/** Tools with at least one folder on this computer, in the backend's order. */
export function foundAgentTools(detected: DetectedSources): AgentToolId[] {
  return inToolOrder(
    agentRows(detected)
      .filter((row) => row.found)
      .map((row) => row.tool),
  )
}

/** Browsers with a history file (or one the system hides), each once, in the order found. */
export function foundBrowsers(detected: DetectedSources): string[] {
  const found = new Set(
    detected.sources.flatMap((source) =>
      source.kind === 'browsing' && source.found && source.profileId
        ? [source.profileId]
        : [],
    ),
  )
  const names = detected.profiles
    .filter((profile) => found.has(profile.profileId))
    .map((profile) => profile.browserName)
  return [...new Set(names)]
}
