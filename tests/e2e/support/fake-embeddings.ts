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
 */
import http from 'node:http'
import type { AddressInfo } from 'node:net'

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
}: {
  dimensions?: number
  /** Requests answered before the rest are held. */
  holdAfter?: number
} = {}): Promise<FakeEmbeddings> {
  const inputs: string[] = []
  let answered = 0
  let waiting: (() => void)[] | null = []
  const server = http.createServer((request, response) => {
    let raw = ''
    request.on('data', (chunk: Buffer) => (raw += chunk.toString('utf8')))
    request.on('end', () => {
      if (request.method !== 'POST' || !request.url?.endsWith('/embeddings')) {
        response.writeHead(404).end()
        return
      }
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
