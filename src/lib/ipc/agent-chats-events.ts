/**
 * The event the backend sends when what the conversation list and reader
 * read has changed: a person changed a conversation's marks, or a backup
 * published conversations.
 *
 * Not responsible for: what to refetch (`src/lib/queries/agent-chats.ts`).
 */
import {
  AGENT_CONVERSATIONS_CHANGED_EVENT,
  type AgentConversationsChangedEvent,
} from '../backend-client/agent-chats'
import { listenDesktopEvent } from './events'

/** Listens until the returned function is called. */
export function subscribeToAgentConversations(
  listener: (event: AgentConversationsChangedEvent) => void,
): Promise<() => void> {
  return listenDesktopEvent(AGENT_CONVERSATIONS_CHANGED_EVENT, listener)
}
