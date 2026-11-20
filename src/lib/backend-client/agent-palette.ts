/**
 * Typed client for ⌘K's Agent conversations and Projects: the six most
 * recent conversations (or those whose title matches), and five projects.
 *
 * Responsible for: the command, its request and the answer's shape. A
 * request carries a fresh id so the next keystroke can call it off
 * (`agentChatsClient.cancel`). Not responsible for: caching or the copy.
 *
 * Source of truth: `docs/architecture/desktop-command-surface.md` (Agent
 * viewer, export and shell). Empty text: the six most recent conversations
 * not put away. Typed text: renamed titles and project names matched
 * without regard to case, put-away conversations included; the trash never.
 */
import type { ChatSummary } from './agent-chats'
import { call } from './shared'

export interface PaletteProject {
  id: string
  /** The name shown everywhere, the person's own if they renamed it. */
  name: string
  mainFolder: string | null
  lastActivityAt: number | null
}

export interface AgentPaletteAnswer {
  conversations: ChatSummary[]
  projects: PaletteProject[]
}

export const agentPaletteClient = {
  query: (text: string, requestId: string) =>
    call<AgentPaletteAnswer>('query_agent_palette', {
      request: { text, requestId },
    }),
}
