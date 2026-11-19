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
        claimed: boolean
        arrived: (offset: number) => void
        resume: Promise<void>
        release: () => void
      }
    | undefined
  const server = http.createServer(async (req, res) => {
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
      for (let offset = 0; offset < bytes.length; offset += chunkBytes) {
        if (
          hold &&
          offset >= (hold.afterBytes ?? (bytes.length <= chunkBytes ? 0 : 1)) &&
          hold.file === file &&
          !hold.claimed
        ) {
          hold.claimed = true
          hold.arrived(offset)
          await hold.resume
        }
        if (res.destroyed) return
        await new Promise<void>((resolve, reject) => {
          res.write(bytes.subarray(offset, offset + chunkBytes), (err) =>
            err ? reject(err) : resolve(),
          )
        })
        if (delayMs > 0) await delay(delayMs)
      }
      res.end()
    } catch {
      res.destroy()
    }
  })
  await new Promise<void>((resolve, reject) => {
    server.once('error', reject)
    server.listen(0, '127.0.0.1', resolve)
  })
  const port = (server.address() as AddressInfo).port
  return {
    requests,
    url: (version: Version) => `http://127.0.0.1:${port}/${version}`,
    holdNextChunk(file: FileName = 'model.safetensors', afterBytes?: number) {
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
