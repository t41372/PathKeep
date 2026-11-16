/**
 * Exercises release CLIs over loopback HTTP with synthetic credentials.
 * Responsibilities: isolate GitHub context, capture requests and clean up the
 * child process/server/output file. Not responsible for release decisions.
 */
import { spawn } from 'node:child_process'
import { mkdtempSync, readFileSync, rmSync } from 'node:fs'
import { createServer } from 'node:http'
import type { AddressInfo } from 'node:net'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

export const sourceSha = 'a'.repeat(40)
export const releaseTag = 'v0.5.0'

export interface ReleaseRequest {
  method: string
  path: string
  body: unknown
  authorization: string | undefined
}

interface Reply {
  status?: number
  body?: unknown
  raw?: string
}

/** Keep negative API cases deterministic without giving a test real credentials. */
export async function runReleaseCli(
  script: string,
  respond: (request: ReleaseRequest) => Reply,
  environment: NodeJS.ProcessEnv = {},
) {
  const folder = mkdtempSync(join(tmpdir(), 'pathkeep-release-cli-'))
  const output = join(folder, 'github-output')
  const requests: ReleaseRequest[] = []
  const server = createServer((request, response) => {
    const chunks: Buffer[] = []
    request.on('data', (chunk: Buffer) => chunks.push(chunk))
    request.on('end', () => {
      const text = Buffer.concat(chunks).toString()
      const record = {
        method: request.method ?? 'GET',
        path: request.url ?? '/',
        body: text ? (JSON.parse(text) as unknown) : null,
        authorization: request.headers.authorization,
      }
      requests.push(record)
      const reply = respond(record)
      response.writeHead(reply.status ?? 200, {
        'content-type': 'application/json',
      })
      response.end(reply.raw ?? JSON.stringify(reply.body ?? {}))
    })
  })
  try {
    await new Promise<void>((done, reject) => {
      server.once('error', reject)
      server.listen(0, '127.0.0.1', done)
    })
    const port = (server.address() as AddressInfo).port
    const inherited = Object.fromEntries(
      Object.entries(process.env).filter(
        ([name]) => !/^(?:GH_|GITHUB_)/i.test(name),
      ),
    )
    const child = spawn(process.execPath, [resolve('scripts', script)], {
      env: {
        ...inherited,
        GITHUB_API_URL: `http://127.0.0.1:${port}`,
        GITHUB_REPOSITORY: 'owner/repo',
        GITHUB_TOKEN: 'fixture-token',
        GITHUB_OUTPUT: output,
        GITHUB_ACTIONS: 'true',
        GITHUB_JOB: 'publish-release',
        IMMUTABLE_RELEASES_CONFIRMED: 'true',
        RESOLVED_TAG: releaseTag,
        SOURCE_COMMIT: sourceSha,
        RELEASE_ID: '41',
        ...environment,
      },
      stdio: ['ignore', 'pipe', 'pipe'],
    })
    let stdout = ''
    let stderr = ''
    child.stdout.on('data', (data: Buffer) => {
      stdout += data.toString()
    })
    child.stderr.on('data', (data: Buffer) => {
      stderr += data.toString()
    })
    const exit = await new Promise<number | null>((done, reject) => {
      child.once('error', reject)
      child.once('close', done)
    })
    let contents = ''
    try {
      contents = readFileSync(output, 'utf8')
    } catch {
      // Failed preparation must not emit a continuation output.
    }
    return { exit, stdout, stderr, contents, requests }
  } finally {
    if (server.listening) {
      await new Promise<void>((done) => server.close(() => done()))
    }
    rmSync(folder, { recursive: true, force: true })
  }
}
