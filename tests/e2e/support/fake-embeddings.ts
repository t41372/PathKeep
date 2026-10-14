/**
 * An OpenAI-compatible embeddings service on 127.0.0.1, for
 * `semantic-index.spec`: each input's vector counts its words, each word in a
 * slot picked by its hash, so texts that share words are close and a search
 * for a page's words finds that page.
 *
 * It can answer a few requests and then hold every later one until
 * `release()`, so a build stops half way for as long as a test needs, with
 * no timing involved. Every text it answered for is kept, in order; a held
 * request whose app went away before the release is never answered and not
 * kept.
 *
 * It answers the rest the way LM Studio does: its models at `/api/v0/models`
 * (with their kind, whether loaded, and their context lengths) and at
 * `/v1/models`, and any other path with HTTP 200 and an `error`, not a 404.
 */
import http from 'node:http'
import type { AddressInfo } from 'node:net'

/** One model as LM Studio's `/api/v0/models` lists it. */
export interface FakeModel {
  id: string
  type: 'llm' | 'vlm' | 'embeddings'
  state: 'loaded' | 'not-loaded'
  max_context_length: number
  loaded_context_length?: number
}

const DEFAULT_MODELS: FakeModel[] = [
  {
    id: 'e2e-embed',
    type: 'embeddings',
    state: 'loaded',
    max_context_length: 2048,
    loaded_context_length: 2048,
  },
]

export interface FakeEmbeddings {
  /** The address to give PathKeep, ending in `/v1`. */
  url: string
  /** Every text answered so far. */
  inputs: string[]
  /** How many requests are waiting for `release()`. */
  held(): number
  /** Answers the held requests and every later one. */
  release(): void
  close(): Promise<void>
}

/** Word counts in hashed slots, scaled to length 1. */
function vectorOf(text: string, dimensions: number) {
  const vector = new Array<number>(dimensions).fill(0)
  for (const word of text.toLowerCase().split(/[^\p{L}\p{N}]+/u)) {
    if (!word) continue
    let hash = 2166136261
    for (const char of word)
      hash = Math.imul(hash ^ char.codePointAt(0)!, 16777619)
    vector[(hash >>> 0) % dimensions] += 1
  }
  const norm = Math.hypot(...vector) || 1
  return vector.map((value) => value / norm)
}

export async function startFakeEmbeddings({
  dimensions = 64,
  holdAfter = Infinity,
  models = DEFAULT_MODELS,
}: {
  dimensions?: number
  /** Requests answered before the rest are held. */
  holdAfter?: number
  models?: FakeModel[]
} = {}): Promise<FakeEmbeddings> {
  const inputs: string[] = []
  let answered = 0
  let waiting: (() => void)[] | null = []
  const server = http.createServer((request, response) => {
    let raw = ''
    request.on('data', (chunk: Buffer) => (raw += chunk.toString('utf8')))
    request.on('end', () => {
      const json = (body: unknown) => {
        response.writeHead(200, { 'content-type': 'application/json' })
        response.end(JSON.stringify(body))
      }
      const path = request.url?.split('?')[0]
      if (request.method === 'GET' && path === '/api/v0/models')
        return json({ object: 'list', data: models })
      if (request.method === 'GET' && path === '/v1/models')
        return json({
          object: 'list',
          data: models.map((model) => ({
            id: model.id,
            object: 'model',
            owned_by: 'e2e',
          })),
        })
      if (request.method !== 'POST' || path !== '/v1/embeddings')
        return json({
          error: `Unexpected endpoint or method. (${request.method} ${path})`,
        })
      const body = JSON.parse(raw || '{}') as {
        model?: string
        input?: string | string[]
      }
      const texts =
        typeof body.input === 'string' ? [body.input] : (body.input ?? [])
      const answer = () => {
        // A killed app's connection is closed by the time it is released.
        if (request.socket.destroyed || !request.socket.writable) return
        answered += 1
        inputs.push(...texts)
        response.writeHead(200, { 'content-type': 'application/json' })
        response.end(
          JSON.stringify({
            object: 'list',
            data: texts.map((text, index) => ({
              object: 'embedding',
              index,
              embedding: vectorOf(text, dimensions),
            })),
            model: body.model ?? 'e2e-embed',
            usage: { prompt_tokens: texts.length, total_tokens: texts.length },
          }),
        )
      }
      if (waiting && answered >= holdAfter) waiting.push(answer)
      else answer()
    })
  })
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
  const { port } = server.address() as AddressInfo
  return {
    url: `http://127.0.0.1:${port}/v1`,
    inputs,
    held: () => waiting?.length ?? 0,
    release: () => {
      const held = waiting ?? []
      waiting = null
      for (const answer of held) answer()
    },
    close: () =>
      new Promise<void>((resolve) => {
        server.closeAllConnections()
        server.close(() => resolve())
      }),
  }
}
