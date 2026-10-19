/**
 * An OpenAI-compatible embeddings service on 127.0.0.1, for
 * `semantic-index.spec`: each input's vector counts its words, each word in a
 * slot picked by its hash, so texts that share words are close and a search
 * for a page's words finds that page. A query in Qwen3-Embedding's form
 * (`Instruct: …\nQuery:…`) counts only the words after `Query:`: the
 * instruction tells a real model what to look for, it is not looked for.
 *
 * It can answer a few requests and then hold every later one until
 * `release()`, so a build stops half way for as long as a test needs, with
 * no timing involved; `holdAfter(n)` does it again later. Every text it
 * answered for is kept, in order; a held request whose app went away before
 * the release is never answered and not kept. `requests()` counts every
 * embeddings request that arrived, answered or not.
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
  /** Every embeddings request that arrived, answered, held or dropped. */
  requests(): number
  /** Answers the held requests and every later one. */
  release(): void
  /** Answers `count` more requests from now on, then holds the rest. */
  holdAfter(count: number): void
  close(): Promise<void>
}

const INSTRUCTED_QUERY = /^Instruct: [^\n]*\nQuery:/

/** Word counts in hashed slots, scaled to length 1. */
export function fakeEmbeddingVector(text: string, dimensions: number) {
  const vector = new Array<number>(dimensions).fill(0)
  const searched = text.replace(INSTRUCTED_QUERY, '')
  for (const word of searched.toLowerCase().split(/[^\p{L}\p{N}]+/u)) {
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
  vectorFor = fakeEmbeddingVector,
}: {
  dimensions?: number
  /** Requests answered before the rest are held. */
  holdAfter?: number
  models?: FakeModel[]
  /** A transport response with a particular edge case, without replacing index computation. */
  vectorFor?: typeof fakeEmbeddingVector
} = {}): Promise<FakeEmbeddings> {
  const inputs: string[] = []
  let answered = 0
  let arrived = 0
  let holdAt = holdAfter
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
      arrived += 1
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
              embedding: vectorFor(text, dimensions),
            })),
            model: body.model ?? 'e2e-embed',
            usage: { prompt_tokens: texts.length, total_tokens: texts.length },
          }),
        )
      }
      if (waiting && answered >= holdAt) waiting.push(answer)
      else answer()
    })
  })
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
  const { port } = server.address() as AddressInfo
  return {
    url: `http://127.0.0.1:${port}/v1`,
    inputs,
    held: () => waiting?.length ?? 0,
    requests: () => arrived,
    release: () => {
      const held = waiting ?? []
      waiting = null
      for (const answer of held) answer()
    },
    holdAfter: (count) => {
      waiting ??= []
      holdAt = answered + count
    },
    close: () =>
      new Promise<void>((resolve) => {
        server.closeAllConnections()
        server.close(() => resolve())
      }),
  }
}
