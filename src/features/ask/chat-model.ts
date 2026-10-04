/**
 * The chat transcript as the UI sees it, and its mapping to the backend's
 * saved conversation format.
 *
 * Responsibilities: turn/step types, building the model transcript, and
 * (de)serialising turns for `save_ai_conversation` / `load_ai_conversation`.
 * Not responsible for streaming (see `use-chat.ts`) or rendering.
 */
import type {
  AgentMessage,
  AiAgentNote,
  AiChatCitation,
  AiChatMessage,
} from '@/lib/types'

export interface ToolStep {
  id: string
  callId?: string
  name: string
  arguments: string
  result?: string
  isError?: boolean
  codeSource?: string
  pending: boolean
}

export type TurnStatus = 'streaming' | 'done' | 'error' | 'cancelled'

export interface ChatTurn {
  id: string
  role: 'user' | 'assistant'
  content: string
  reasoning: string
  tools: ToolStep[]
  citations: AiChatCitation[]
  notes: AiAgentNote[]
  usage?: { promptTokens: number; completionTokens: number }
  status: TurnStatus
  error?: string
}

export function newId(prefix: string) {
  return `${prefix}-${crypto.randomUUID()}`
}

export function userTurn(content: string): ChatTurn {
  return emptyTurn(newId('user'), 'user', content, 'done')
}

export function assistantTurn(): ChatTurn {
  return emptyTurn(newId('assistant'), 'assistant', '', 'streaming')
}

function emptyTurn(
  id: string,
  role: ChatTurn['role'],
  content: string,
  status: TurnStatus,
): ChatTurn {
  return {
    id,
    role,
    content,
    reasoning: '',
    tools: [],
    citations: [],
    notes: [],
    status,
  }
}

/** Only finished text goes back to the model; tool traces stay on our side. */
export function buildTranscript(
  turns: readonly ChatTurn[],
  systemPrompt: string,
): AiChatMessage[] {
  const messages: AiChatMessage[] = []
  if (systemPrompt.trim())
    messages.push({ role: 'system', content: systemPrompt })
  for (const turn of turns) {
    if (turn.role === 'user') {
      messages.push({ role: 'user', content: turn.content })
    } else if (turn.content) {
      messages.push({ role: 'assistant', content: turn.content })
    }
  }
  return messages
}

interface StoredTrace {
  tools: ToolStep[]
  notes: AiAgentNote[]
}

export function toAgentMessage(turn: ChatTurn): AgentMessage {
  const hasTrace = turn.tools.length > 0 || turn.notes.length > 0
  const trace: StoredTrace = { tools: turn.tools, notes: turn.notes }
  return {
    id: turn.id,
    role: turn.role,
    content: turn.content,
    reasoning: turn.reasoning || null,
    toolCallsJson: hasTrace ? JSON.stringify(trace) : null,
    status: turn.role === 'assistant' ? turn.status : null,
  }
}

function parseTrace(json: string | null | undefined): StoredTrace {
  if (!json) return { tools: [], notes: [] }
  try {
    const parsed: unknown = JSON.parse(json)
    if (Array.isArray(parsed)) return { tools: parsed as ToolStep[], notes: [] }
    if (parsed && typeof parsed === 'object') {
      const trace = parsed as Partial<StoredTrace>
      return { tools: trace.tools ?? [], notes: trace.notes ?? [] }
    }
  } catch {
    // A trace we cannot read is dropped; the answer text is still shown.
  }
  return { tools: [], notes: [] }
}

export function fromAgentMessage(message: AgentMessage): ChatTurn {
  const isAssistant = message.role === 'assistant'
  const trace = parseTrace(message.toolCallsJson)
  const status = (message.status as TurnStatus | null | undefined) ?? 'done'
  return {
    id: message.id,
    role: isAssistant ? 'assistant' : 'user',
    content: message.content,
    reasoning: message.reasoning ?? '',
    // A trace saved mid-call would otherwise spin forever on reopen.
    tools: trace.tools.map((tool) => ({ ...tool, pending: false })),
    citations: (message.citations ?? []).map((citation) => ({
      historyId: citation.historyId,
      profileId: citation.profileId,
      url: citation.url,
      title: citation.title ?? null,
      visitedAt: citation.visitedAt,
      score: citation.score ?? null,
      canonicalUrl: citation.canonicalUrl ?? null,
    })),
    notes: trace.notes,
    usage: message.usage ?? undefined,
    status: isAssistant
      ? status === 'streaming'
        ? 'cancelled'
        : status
      : 'done',
  }
}
