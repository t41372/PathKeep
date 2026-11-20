/**
 * Typed client for the Agent preferences that belong to this computer, not
 * to the archive (`agent-history.md` §7): how the reader opens tool calls
 * and thinking, which folders count as scratch ("loose folders" in
 * Projects), and the three switches that let AI read conversations.
 *
 * Responsible for: command names, the request envelope, the wire shape and
 * the change event. Not responsible for: caching
 * (`src/lib/queries/agent-viewer.ts`) or any copy. The two cost switches
 * live with the archive and go through `agent-insights.ts`.
 *
 * Source of truth: `docs/architecture/desktop-command-surface.md` (Agent
 * viewer, export and shell).
 */
import { call } from './shared'

export interface AgentViewerPreferences {
  /**
   * Folders whose conversations go to the loose folders instead of a
   * project of their own: whole path components and everything under them;
   * the home folder itself matches exactly. Sorted by the backend.
   */
  scratchLocations: string[]
  /** Each turn's tool calls start as one line. */
  readerCollapseToolCalls: boolean
  /** The reader shows the "Thought for …" line where thinking was recorded. */
  readerShowThinking: boolean
  /** Ask may quote Agent conversations. */
  assistantMayRead: boolean
  /** Conversations get a semantic search index. */
  buildSemanticIndex: boolean
  /** Tools connected over MCP may search and read conversations. */
  mcpMayRead: boolean
  /** The price table PathKeep estimates costs with. */
  priceTableVersion: string
}

/** Only the keys named change; the backend refuses keys it does not know. */
export type AgentViewerPatch = Partial<
  Omit<AgentViewerPreferences, 'priceTableVersion'>
>

/** Sent with the whole object after every saved change. */
export const AGENT_VIEWER_PREFERENCES_CHANGED_EVENT =
  'pathkeep://agent-viewer-preferences-changed'

export const agentViewerClient = {
  preferences: () =>
    call<AgentViewerPreferences>('get_agent_viewer_preferences', {
      request: {},
    }),
  /** The complete preferences after the change. */
  setPreferences: (request: AgentViewerPatch) =>
    call<AgentViewerPreferences>('set_agent_viewer_preferences', { request }),
}
