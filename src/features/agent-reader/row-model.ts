/**
 * What a row of the conversation is, for drawing: a prompt, a reply, a
 * thought, tool calls, or something only the raw records show; and the
 * tool calls of one or more rows put together, each with its input, its
 * result and how it went.
 *
 * Rows that hold nothing but tool calls and their results join the rows
 * next to them that do the same, so one step of work reads as one line
 * (`agent-history.md` §4.5); a row that also holds text keeps its calls to
 * itself. A run is drawn at its first row; the rows after it draw nothing.
 * A run whose first row is not read yet starts at the first row that is.
 *
 * Responsibilities: those decisions. Not responsible for: drawing.
 */
import type {
  ChatChildCard,
  ChatEdit,
  ChatRow,
} from '@/lib/backend-client/agent-chats'

export function hasText(row: ChatRow) {
  return row.parts.some((part) => part.kind === 'text' && part.text.trim())
}

function hasCalls(row: ChatRow) {
  return (
    row.parts.some(
      (part) => part.kind === 'callInput' || part.kind === 'callResult',
    ) || row.operations.some((op) => op.kind === 'call' || op.kind === 'result')
  )
}

/** A row that is only tool calls and results: it joins its neighbours. */
export function isToolOnly(row: ChatRow) {
  return (
    hasCalls(row) &&
    !hasText(row) &&
    !row.parts.some((p) => p.kind === 'thinking')
  )
}

/** A person's prompt: their words, not a tool's result handed back as "user". */
export function isPrompt(row: ChatRow) {
  return (
    row.role === 'user' &&
    hasText(row) &&
    !row.parts.some((part) => part.kind === 'callResult')
  )
}

/** Where the parts of `kind` are in `row.parts`. */
export function partIndexes(
  row: ChatRow,
  kind: ChatRow['parts'][number]['kind'],
) {
  const indexes: number[] = []
  row.parts.forEach((part, index) => {
    if (part.kind === kind) indexes.push(index)
  })
  return indexes
}

/** Find's tokens for the parts of `kind` (`find-target.ts`). */
export function partTokensOf(
  row: ChatRow,
  kind: ChatRow['parts'][number]['kind'],
) {
  return partIndexes(row, kind)
    .map((index) => `${row.sourceId}:${row.ordinal}:${index}`)
    .join(' ')
}

export function textOf(row: ChatRow) {
  return row.parts
    .filter((part) => part.kind === 'text')
    .map((part) => part.text)
    .join('\n\n')
}

export function thinkingOf(row: ChatRow) {
  return row.parts
    .filter((part) => part.kind === 'thinking')
    .map((part) => part.text)
    .join('\n\n')
}

export function thinkingMs(row: ChatRow) {
  let total: number | null = null
  for (const op of row.operations)
    if (op.kind === 'thinking') total = (total ?? 0) + op.durationMs
  return total
}

export interface ToolCall {
  key: string
  /** `null` for a result whose call is in a row not read. */
  name: string | null
  input: string | null
  output: string | null
  status: 'ok' | 'error' | 'denied' | null
  durationMs: number | null
  edit: ChatEdit | null
  /** The output was cut to fit the window. */
  truncated: boolean
  /** Find's tokens for its input and its output part. */
  inputToken: string | null
  outputToken: string | null
}

/** The calls of `rows` in order, each result joined to its call. */
export function toolCalls(rows: ChatRow[]): ToolCall[] {
  const calls: ToolCall[] = []
  const byId = new Map<string, ToolCall>()
  for (const row of rows) {
    const token = (index: number | undefined) =>
      index === undefined ? null : `${row.sourceId}:${row.ordinal}:${index}`
    const inputIndexes = partIndexes(row, 'callInput')
    const outputIndexes = partIndexes(row, 'callResult')
    const inputs = inputIndexes.map((index) => row.parts[index])
    const outputs = outputIndexes.map((index) => row.parts[index])
    let input = 0
    let output = 0
    for (const op of row.operations) {
      if (op.kind === 'call') {
        const call: ToolCall = {
          key: `${row.sourceId}:${row.ordinal}:${calls.length}`,
          name: op.name,
          input: inputs[input]?.text ?? null,
          output: null,
          status: op.status ?? null,
          durationMs: op.durationMs ?? null,
          edit: op.edit ?? null,
          truncated: false,
          inputToken: token(inputIndexes[input]),
          outputToken: null,
        }
        input += 1
        calls.push(call)
        if (op.callId) byId.set(op.callId, call)
      } else if (op.kind === 'result') {
        const text = outputs[output]?.text ?? null
        const outputToken = token(outputIndexes[output])
        output += 1
        const call = op.callId ? byId.get(op.callId) : undefined
        if (call) {
          call.output = text
          call.outputToken = outputToken
          call.status ??= op.status
          call.durationMs ??= op.durationMs ?? null
          call.truncated = row.truncated
        } else {
          calls.push({
            key: `${row.sourceId}:${row.ordinal}:${calls.length}`,
            name: null,
            input: null,
            output: text,
            status: op.status,
            durationMs: op.durationMs ?? null,
            edit: null,
            truncated: row.truncated,
            inputToken: null,
            outputToken,
          })
        }
      }
    }
  }
  return calls
}

/** One line out of a call's input: the path, command or pattern it names. */
export function callArgument(input: string | null) {
  if (!input) return ''
  try {
    const value = JSON.parse(input) as unknown
    if (value && typeof value === 'object' && !Array.isArray(value)) {
      const record = value as Record<string, unknown>
      for (const key of [
        'file_path',
        'path',
        'command',
        'cmd',
        'pattern',
        'query',
        'url',
        'description',
        'prompt',
      ]) {
        const field = record[key]
        if (typeof field === 'string' && field.trim()) return oneLine(field)
        if (
          Array.isArray(field) &&
          field.every((item) => typeof item === 'string')
        )
          return oneLine(field.join(' '))
      }
    }
  } catch {
    // Not JSON: the input itself.
  }
  return oneLine(input)
}

function oneLine(text: string) {
  return text.replace(/\s+/g, ' ').trim().slice(0, 200)
}

/** Where each subagent was sent off: `sourceId:ordinal` → its cards. */
export function launchesOf(children: ChatChildCard[]) {
  const map = new Map<string, ChatChildCard[]>()
  for (const child of children) {
    if (!child.launch) continue
    const key = `${child.launch.sourceId}:${child.launch.ordinal}`
    const list = map.get(key)
    if (list) list.push(child)
    else map.set(key, [child])
  }
  return map
}

export function rowKey(row: Pick<ChatRow, 'sourceId' | 'ordinal'>) {
  return `${row.sourceId}:${row.ordinal}`
}

/** The turn a row belongs to, `sourceId:turnOrdinal`; `null` before the first prompt. */
export function turnKey(row: Pick<ChatRow, 'sourceId' | 'turnOrdinal'>) {
  return row.turnOrdinal === null ? null : `${row.sourceId}:${row.turnOrdinal}`
}

/** The reading column: a line length people can read (`agent-history.md` §4.5). */
export const COLUMN = 'mx-auto max-w-[760px] px-6 py-1.5'
