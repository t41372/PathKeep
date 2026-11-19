/**
 * Start before launching a debug desktop process:
 *
 *   const model = await startBuiltinModelSource({chunkBytes: 8192, delayMs: 5})
 *   // Set PATHKEEP_TEST_MODEL_SOURCE=model.url('v1') in the desktop process environment.
 *   const hold = model.holdNextChunk('model.safetensors', 1024 * 1024)
 *   // Start the real Settings download; await hold.held before cancelling/restarting.
 *   // held resolves with the bytes sent. hold.release() resumes the response.
 *   // afterBytes is optional. Without it the first chunk is sent before the hold
 *   // (only the headers for a one-chunk file). Arm before starting the transfer.
 *   // model.requests records each GET. Close with await model.close() in finally.
 *
 * The app gives up on a response that sends nothing for 30 seconds (it then
 * ends with `error`, or `cancelled` once a cancel was taken), and it sees a
 * cancel only when a read returns. So a plain hold stops everything, cancels
 * included, for a few seconds at most. `{keepAlive: true}` instead sends one
 * byte of the held chunk every 10 seconds, never its last, so the download
 * stays alive as long as needed: the app hears no progress (it reports every
 * MB), but it sees a cancel at the next byte.
 *
 * Reopen the same archive with model.url('v2') to replace v1: only the weights have
 * different pins. A cancelled download starts a fresh GET; a held response resumes in
 * place. The SHA-256 manifest and the 256-dimensional model2vec fixtures live together
 * in vault-core/tests/fixtures/builtin-model. The matrix crosses one byte-progress stride.
 */
import http from 'node:http'
import { readFile } from 'node:fs/promises'
import type { AddressInfo } from 'node:net'
import { setTimeout as delay } from 'node:timers/promises'

type Version = 'v1' | 'v2'

/** Under the app's 30-second read timeout, with room to spare. */
const KEEP_ALIVE_MS = 10_000
type FileName = 'config.json' | 'tokenizer.json' | 'model.safetensors'

export async function startBuiltinModelSource({
  chunkBytes = 8192,
  delayMs = 0,
}: { chunkBytes?: number; delayMs?: number } = {}) {
  if (
    !Number.isSafeInteger(chunkBytes) ||
    chunkBytes < 1 ||
    !Number.isFinite(delayMs) ||
    delayMs < 0
  )
    throw new RangeError('Invalid model response chunk size or delay')
  const fixtures = new URL(
    '../../../src-tauri/crates/vault-core/tests/fixtures/builtin-model/',
    import.meta.url,
  )
  const requests: string[] = []
  let hold:
    | {
        file: FileName
        afterBytes?: number
        keepAlive: boolean
        claimed: boolean
        arrived: (offset: number) => void
        resume: Promise<void>
        release: () => void
      }
    | undefined
  const respond = async (
    req: http.IncomingMessage,
    res: http.ServerResponse,
  ) => {
    const match =
      /^\/(v1|v2)\/(config\.json|tokenizer\.json|model\.safetensors)$/.exec(
        req.url ?? '',
      )
    if (req.method !== 'GET' || !match) {
      res.writeHead(404).end()
      return
    }
    const [, version, file] = match
    requests.push(match[0])
    try {
      const bytes = await readFile(
        new URL(
          `${file === 'model.safetensors' ? version : 'common'}/${file}`,
          fixtures,
        ),
      )
      res.writeHead(200, {
        'content-length': bytes.length,
        connection: 'close',
      })
      res.flushHeaders()
      const send = (part: Uint8Array) =>
        new Promise<void>((resolve, reject) => {
          res.write(part, (err) => (err ? reject(err) : resolve()))
        })
      for (let offset = 0; offset < bytes.length; offset += chunkBytes) {
        let chunk = bytes.subarray(offset, offset + chunkBytes)
        const held = hold
        if (
          held &&
          offset >= (held.afterBytes ?? (bytes.length <= chunkBytes ? 0 : 1)) &&
          held.file === file &&
          !held.claimed
        ) {
          held.claimed = true
          held.arrived(offset)
          let released = false
          void held.resume.then(() => {
            released = true
          })
          while (held.keepAlive && !released && chunk.length > 1) {
            await Promise.race([held.resume, delay(KEEP_ALIVE_MS)])
            if (released || res.destroyed) break
            await send(chunk.subarray(0, 1))
            chunk = chunk.subarray(1)
          }
          await held.resume
        }
        if (res.destroyed) return
        await send(chunk)
        if (delayMs > 0) await delay(delayMs)
      }
      res.end()
    } catch {
      res.destroy()
    }
  }
  const server = http.createServer((req, res) => {
    void respond(req, res)
  })
  await new Promise<void>((resolve, reject) => {
    server.once('error', reject)
    server.listen(0, '127.0.0.1', resolve)
  })
  const port = (server.address() as AddressInfo).port
  return {
    requests,
    url: (version: Version) => `http://127.0.0.1:${port}/${version}`,
    holdNextChunk(
      file: FileName = 'model.safetensors',
      afterBytes?: number,
      { keepAlive = false }: { keepAlive?: boolean } = {},
    ) {
      if (
        afterBytes !== undefined &&
        (!Number.isSafeInteger(afterBytes) || afterBytes < 0)
      )
        throw new RangeError('Invalid hold offset')
      if (hold) throw new Error('A response hold is already armed')
      let arrived!: (offset: number) => void
      let resume!: () => void
      const held = new Promise<number>((resolve) => {
        arrived = resolve
      })
      const wait = new Promise<void>((resolve) => {
        resume = resolve
      })
      const release = () => {
        if (hold?.release === release) hold = undefined
        resume()
      }
      hold = {
        file,
        afterBytes,
        keepAlive,
        claimed: false,
        arrived,
        resume: wait,
        release,
      }
      return { held, release }
    },
    async close() {
      hold?.release()
      const closed = new Promise<void>((resolve, reject) =>
        server.close((err) => (err ? reject(err) : resolve())),
      )
      server.closeAllConnections()
      await closed
    },
  }
}
