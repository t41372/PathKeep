/**
 * An OpenAI-compatible chat service on 127.0.0.1, for the specs that set up
 * an AI service (Ask, Settings → AI's connection test). It answers the way a
 * model that uses PathKeep's tools would, and nothing more:
 *
 * - asked with tools, it calls `search_history` for what the spec told it to
 *   search for;
 * - given that tool's result, it answers with one fixed sentence;
 * - asked without tools (the connection test), it says OK.
 *
 * It streams when asked to, the way OpenAI does (`chat.completion.chunk`
 * events, then a usage event, then `[DONE]`). A request without the right
 * `Bearer` key gets OpenAI's 401. Every request is kept, so a spec can check
 * what PathKeep sent.
 */
import http from 'node:http'
import type { AddressInfo } from 'node:net'

/** What the fake model says once it has the search's results. */
export const FAKE_ANSWER = 'You read about it on the page below.'

export interface FakeRequest {
  path: string
  authorization: string | undefined
  body: {
    model?: string
    stream?: boolean
    tools?: { function?: { name?: string } }[]
    messages?: { role: string; content: unknown }[]
  }
}

export interface FakeModel {
  /** The address to give PathKeep, ending in `/v1`. */
  url: string
  requests: FakeRequest[]
  close(): Promise<void>
}

interface Reply {
  content?: string
  toolCall?: { name: string; arguments: string }
}

function reply(body: FakeRequest['body'], searchFor: string): Reply {
  const last = body.messages?.at(-1)
  if (last?.role === 'tool') return { content: FAKE_ANSWER }
  const tools = (body.tools ?? []).map((tool) => tool.function?.name)
  if (tools.includes('search_history'))
    return {
      toolCall: {
        name: 'search_history',
        arguments: JSON.stringify({ query: searchFor }),
      },
    }
  return { content: 'OK' }
}

const usage = { prompt_tokens: 12, completion_tokens: 8, total_tokens: 20 }

function stream(
  response: http.ServerResponse,
  model: string,
  { content, toolCall }: Reply,
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
  delta({ role: 'assistant', content: '' })
  if (toolCall) {
    delta({
      tool_calls: [
        {
          index: 0,
          id: 'call_e2e',
          type: 'function',
          function: toolCall,
        },
      ],
    })
    delta({}, 'tool_calls')
  } else {
    for (const word of (content ?? '').split(/(?<= )/)) delta({ content: word })
    delta({}, 'stop')
  }
  send({ choices: [], usage })
  response.end('data: [DONE]\n\n')
}

function complete(
  response: http.ServerResponse,
  model: string,
  { content, toolCall }: Reply,
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
          message: toolCall
            ? {
                role: 'assistant',
                content: null,
                tool_calls: [
                  { id: 'call_e2e', type: 'function', function: toolCall },
                ],
              }
            : { role: 'assistant', content },
          finish_reason: toolCall ? 'tool_calls' : 'stop',
        },
      ],
      usage,
    }),
  )
}

/** Starts the service; `searchFor` is what it asks PathKeep's search for. */
export async function startFakeModel({
  apiKey,
  searchFor,
}: {
  apiKey: string
  searchFor: string
}): Promise<FakeModel> {
  const requests: FakeRequest[] = []
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
      if (request.headers.authorization !== `Bearer ${apiKey}`) {
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
      if (
        request.method !== 'POST' ||
        !request.url?.endsWith('/chat/completions')
      ) {
        response.writeHead(404)
        response.end()
        return
      }
      const answer = reply(body, searchFor)
      const model = body.model ?? 'e2e-model'
      if (body.stream) stream(response, model, answer)
      else complete(response, model, answer)
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
