/**
 * @file index.ts
 * @description Public barrel for the streaming assistant-chat surface (W-AI-2).
 * @module components/assistant-chat
 *
 * Re-exports the hook, composed view, copy builders, and block primitives so the assistant
 * route imports from one place. Excluded from coverage `include` only as a re-export shim
 * (see vitest.config.ts precedent for the other component barrels).
 */

export { useAiChatStream } from './use-ai-chat-stream'
export type {
  AiChatStreamDeps,
  AiChatStreamState,
  AssistantToolCall,
  AssistantTurnStatus,
  ChatMessage,
} from './use-ai-chat-stream'

export { AssistantChatView } from './assistant-chat-view'
export type {
  AssistantChatComposerCopy,
  AssistantChatViewCopy,
  AssistantChatViewProps,
} from './assistant-chat-view'

export { AssistantTurn } from './assistant-turn'
export type { AssistantTurnCopy, AssistantTurnProps } from './assistant-turn'

export { ReasoningBlock } from './reasoning-block'
export type { ReasoningBlockCopy, ReasoningBlockProps } from './reasoning-block'

export { ToolCallBlock } from './tool-call-block'
export type { ToolCallBlockCopy, ToolCallBlockProps } from './tool-call-block'

export { StreamingMarkdown } from './streaming-markdown'
export type { StreamingMarkdownProps } from './streaming-markdown'

export {
  buildAssistantChatCopy,
  buildAssistantChatPrompts,
} from './assistant-chat-copy'
export type {
  AssistantTranslator,
  BuildAssistantChatCopyOptions,
} from './assistant-chat-copy'

export { ChatHistoryExplorer } from './chat-history-explorer'
export type {
  ChatHistoryCopy,
  ChatHistoryExplorerProps,
} from './chat-history-explorer'

export { buildChatHistoryCopy, formatRelativeTime } from './chat-history-copy'

export { useChatHistory } from './use-chat-history'
export type {
  ChatHistoryBackend,
  ChatHistoryState,
  UseChatHistoryOptions,
} from './use-chat-history'
