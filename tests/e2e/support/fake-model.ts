/**
 * An OpenAI-compatible chat service on 127.0.0.1, for the specs that set up
 * an AI service (Ask, Settings → AI's connection test). By default it answers
 * the way a model that uses PathKeep's tools would, and nothing more:
 *
 * - asked with tools, it calls `search_history` for what the spec told it to
 *   search for;
 * - given that tool's result, it answers with one fixed sentence;
 * - asked without tools (the connection test), it says OK.
 *
 * A spec can script the replies instead (`script`): each request gets what
 * the script returns for it, which may be thinking (`reasoning_content`, as
 * reasoning models served by LM Studio, Ollama or DeepSeek send it), text, or
 * calls to the tools the request offered.
 *
 * It streams when asked to, the way OpenAI does (`chat.completion.chunk`
 * events, then a usage event, then `[DONE]`), and gives every tool call its
 * own id. A request without the right `Bearer` key gets OpenAI's 401 (a
 * service started without a key, as LM Studio runs, takes any). Every
 * request is kept, so a spec can check what PathKeep sent.
 *
 * Given `models`, it also lists them the way LM Studio does
 * (`/api/v0/models`, with each one's state and context lengths) and at
 * `/v1/models`, so Ask's model picker and context meter read them.
 */
import http from 'node:http'
import type { AddressInfo } from 'node:net'

/** What the fake model says once it has the search's results. */
export const FAKE_ANSWER = 'You read about it on the page below.'

export interface ChatMessage {
  role: string
  /** A string, or OpenAI's list of `{ type: 'text', text }` parts. */
  content: unknown
  tool_call_id?: string
  tool_calls?: { id?: string; function?: { name?: string } }[]
}

export interface FakeRequest {
  path: string
  authorization: string | undefined
  body: {
    model?: string
    stream?: boolean
    tools?: { function?: { name?: string } }[]
    messages?: ChatMessage[]
  }
}

export interface FakeModel {
  /** The address to give PathKeep, ending in `/v1`. */
  url: string
  requests: FakeRequest[]
  close(): Promise<void>
}

/** One reply of the model. */
export interface Reply {
  /** Thinking, sent as `reasoning_content` before anything else. */
  reasoning?: string
  content?: string
  toolCalls?: { name: string; arguments: object }[]
}

/** Decides the reply to one chat request. */
export type Script = (body: FakeRequest['body']) => Reply

/** One model as LM Studio's `/api/v0/models` lists it. */
export interface ListedModel {
  id: string
  type?: 'llm' | 'vlm' | 'embeddings'
  state?: 'loaded' | 'not-loaded'
  loaded_context_length?: number
  max_context_length?: number
}

/** A message's text, whichever of OpenAI's two shapes it came in. */
export function textOf(content: unknown): string {
  if (typeof content === 'string') return content
  if (Array.isArray(content))
    return content
      .map((part: { text?: unknown }) =>
        typeof part?.text === 'string' ? part.text : '',
      )
      .join('')
  return ''
}

/** Whether the request offered the model any tools. */
export function offersTools(body: FakeRequest['body']) {
  return (body.tools ?? []).length > 0
}

/** The text of the request's last user message. */
export function lastUserText(body: FakeRequest['body']) {
  const user = body.messages?.findLast((message) => message.role === 'user')
  return textOf(user?.content)
}

/** What PathKeep's tools returned, in the order the request carries them. */
export function toolResults(body: FakeRequest['body']) {
  return (body.messages ?? [])
    .filter((message) => message.role === 'tool')
    .map((message) => textOf(message.content))
}

/** The chat requests (not the model list or other paths), in the order they came. */
export function chatRequests(model: FakeModel) {
  return model.requests.filter((request) =>
    request.path.endsWith('/chat/completions'),
  )
}

/** The default behaviour: search for `searchFor`, then answer `FAKE_ANSWER`. */
function searchThenAnswer(searchFor: string): Script {
  return (body) => {
    if (body.messages?.at(-1)?.role === 'tool') return { content: FAKE_ANSWER }
    const tools = (body.tools ?? []).map((tool) => tool.function?.name)
    if (tools.includes('search_history'))
      return {
        toolCalls: [
          { name: 'search_history', arguments: { query: searchFor } },
        ],
      }
    return { content: 'OK' }
  }
}

const usage = { prompt_tokens: 12, completion_tokens: 8, total_tokens: 20 }

interface Call {
  id: string
  type: 'function'
  function: { name: string; arguments: string }
}

function stream(
  response: http.ServerResponse,
  model: string,
  { reasoning, content }: Reply,
  calls: Call[],
) {
  response.writeHead(200, {
    'content-type': 'text/event-stream',
    'cache-control': 'no-cache',
  })
  const base = {
    id: 'chatcmpl-e2e',
    object: 'chat.completion.chunk',
    created: Math.floor(Date.now() / 1000),
    model,
  }
  const send = (event: object) =>
    response.write(`data: ${JSON.stringify({ ...base, ...event })}\n\n`)
  const delta = (value: object, finish: string | null = null) =>
    send({ choices: [{ index: 0, delta: value, finish_reason: finish }] })
  const words = (text: string) => text.split(/(?<= )/)
  delta({ role: 'assistant', content: '' })
  for (const word of words(reasoning ?? '')) delta({ reasoning_content: word })
  for (const word of words(content ?? '')) if (word) delta({ content: word })
  if (calls.length > 0) {
    delta({
      tool_calls: calls.map((call, index) => ({ index, ...call })),
    })
    delta({}, 'tool_calls')
  } else {
    delta({}, 'stop')
  }
  send({ choices: [], usage })
  response.end('data: [DONE]\n\n')
}

function complete(
  response: http.ServerResponse,
  model: string,
  { reasoning, content }: Reply,
  calls: Call[],
) {
  response.writeHead(200, { 'content-type': 'application/json' })
  response.end(
    JSON.stringify({
      id: 'chatcmpl-e2e',
      object: 'chat.completion',
      created: Math.floor(Date.now() / 1000),
      model,
      choices: [
        {
          index: 0,
          message: {
            role: 'assistant',
            content: content ?? null,
            ...(reasoning ? { reasoning_content: reasoning } : {}),
            ...(calls.length > 0 ? { tool_calls: calls } : {}),
          },
          finish_reason: calls.length > 0 ? 'tool_calls' : 'stop',
        },
      ],
      usage,
    }),
  )
}

/**
 * Starts the service. Either `searchFor` (what it asks PathKeep's search for
 * before answering `FAKE_ANSWER`) or a `script` for every reply.
 */
export async function startFakeModel(
  options: { apiKey: string | null; models?: ListedModel[] } & (
    | { searchFor: string; script?: never }
    | { script: Script; searchFor?: never }
  ),
): Promise<FakeModel> {
  const { apiKey } = options
  const script = options.script ?? searchThenAnswer(options.searchFor)
  const requests: FakeRequest[] = []
  let callCount = 0
  const server = http.createServer((request, response) => {
    let raw = ''
    request.on('data', (chunk: Buffer) => (raw += chunk.toString('utf8')))
    request.on('end', () => {
      const body = (raw ? JSON.parse(raw) : {}) as FakeRequest['body']
      requests.push({
        path: request.url ?? '',
        authorization: request.headers.authorization,
        body,
      })
      if (
        apiKey !== null &&
        request.headers.authorization !== `Bearer ${apiKey}`
      ) {
        response.writeHead(401, { 'content-type': 'application/json' })
        response.end(
          JSON.stringify({
            error: {
              message: 'Incorrect API key provided.',
              type: 'invalid_request_error',
              code: 'invalid_api_key',
            },
          }),
        )
        return
      }
      const listed = options.models ?? []
      if (request.method === 'GET' && request.url === '/api/v0/models') {
        response.writeHead(200, { 'content-type': 'application/json' })
        response.end(
          JSON.stringify({
            object: 'list',
            data: listed.map((model) => ({
              type: 'llm',
              state: 'loaded',
              max_context_length: model.loaded_context_length,
              ...model,
              object: 'model',
            })),
          }),
        )
        return
      }
      if (request.method === 'GET' && request.url === '/v1/models') {
        response.writeHead(200, { 'content-type': 'application/json' })
        response.end(
          JSON.stringify({
            object: 'list',
            data: listed.map(({ id }) => ({ id, object: 'model' })),
          }),
        )
        return
      }
      if (
        request.method !== 'POST' ||
        !request.url?.endsWith('/chat/completions')
      ) {
        response.writeHead(404)
        response.end()
        return
      }
      const reply = script(body)
      const calls: Call[] = (reply.toolCalls ?? []).map((call) => ({
        id: `call_${++callCount}`,
        type: 'function',
        function: {
          name: call.name,
          arguments: JSON.stringify(call.arguments),
        },
      }))
      const model = body.model ?? 'e2e-model'
      if (body.stream) stream(response, model, reply, calls)
      else complete(response, model, reply, calls)
    })
  })
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
  const { port } = server.address() as AddressInfo
  return {
    url: `http://127.0.0.1:${port}/v1`,
    requests,
    close: () =>
      new Promise<void>((resolve) => {
        server.closeAllConnections()
        server.close(() => resolve())
      }),
  }
}
