/**
 * Protects published binaries from a repeated release workflow.
 * Responsibilities: exercise the guard CLI through a real HTTP connection and
 * verify its GitHub Actions output. Only the remote GitHub API is controlled.
 * Not responsible for: building bundles or publishing a release.
 *
 * Failure modes written before implementation:
 * - an existing public tag reaches the build and asset-upload jobs;
 * - a new tag or a draft is mistaken for an already published release;
 * - an API, authentication or metadata error is treated as permission to build;
 * - a missing credential reaches the network or writes a continuation output.
 * These safety decisions need isolated failure injection. A real Actions
 * rehearsal also checks the workflow's job conditions before publication.
 */
import { spawn } from 'node:child_process'
import { mkdtempSync, readFileSync, rmSync } from 'node:fs'
import { createServer } from 'node:http'
import type { AddressInfo } from 'node:net'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { expect, it } from 'vitest'

const script = resolve('scripts/guard-published-release.mjs')
const tag = 'v0.5.0'

interface Scenario {
  name: string
  status: number
  body?: unknown
  raw?: string
  published?: boolean
}

const scenarios: Scenario[] = [
  {
    name: 'a published release skips build and upload',
    status: 200,
    body: { tag_name: tag, draft: false },
    published: true,
  },
  {
    name: 'a draft can continue preparation',
    status: 200,
    body: { tag_name: tag, draft: true },
    published: false,
  },
  {
    name: 'a new tag can continue preparation',
    status: 404,
    body: { message: 'Not Found' },
    published: false,
  },
  { name: 'an API outage blocks preparation', status: 503 },
  { name: 'an authentication failure blocks preparation', status: 401 },
  { name: 'invalid JSON blocks preparation', status: 200, raw: '{' },
  {
    name: 'metadata for another tag blocks preparation',
    status: 200,
    body: { tag_name: 'v0.4.0', draft: false },
  },
  {
    name: 'an unknown publication state blocks preparation',
    status: 200,
    body: { tag_name: tag, draft: 'false' },
  },
]

async function runGuard(scenario: Scenario, token: string | undefined) {
  const folder = mkdtempSync(join(tmpdir(), 'pathkeep-release-guard-'))
  const output = join(folder, 'github-output')
  const requests: {
    path: string | undefined
    authorization: string | undefined
  }[] = []
  const server = createServer((request, response) => {
    requests.push({
      path: request.url,
      authorization: request.headers.authorization,
    })
    response.writeHead(scenario.status, { 'content-type': 'application/json' })
    response.end(scenario.raw ?? JSON.stringify(scenario.body ?? {}))
  })
  try {
    await new Promise<void>((resolve, reject) => {
      server.once('error', reject)
      server.listen(0, '127.0.0.1', resolve)
    })
    const port = (server.address() as AddressInfo).port
    const child = spawn(process.execPath, [script], {
      env: {
        ...process.env,
        GITHUB_API_URL: `http://127.0.0.1:${port}`,
        GITHUB_REPOSITORY: 'owner/repo',
        GITHUB_TOKEN: token,
        GITHUB_OUTPUT: output,
        RESOLVED_TAG: tag,
      },
      stdio: ['ignore', 'pipe', 'pipe'],
    })
    let stderr = ''
    child.stderr.on('data', (data: Buffer) => {
      stderr += data.toString()
    })
    // Drain stdout even though the contract is the output file and exit status.
    child.stdout.resume()
    const exit = await new Promise<number | null>((resolve, reject) => {
      child.once('error', reject)
      child.once('close', resolve)
    })
    let contents = ''
    try {
      contents = readFileSync(output, 'utf8')
    } catch {
      // A failed lookup must leave the continuation output absent.
    }
    return { exit, contents, requests, stderr }
  } finally {
    if (server.listening) {
      await new Promise<void>((resolve) => server.close(() => resolve()))
    }
    rmSync(folder, { recursive: true, force: true })
  }
}

it.each(scenarios)('$name', async (scenario) => {
  const result = await runGuard(scenario, 'fixture-token')
  expect(result.requests).toEqual([
    {
      path: '/repos/owner/repo/releases/tags/v0.5.0',
      authorization: 'Bearer fixture-token',
    },
  ])
  if (scenario.published === undefined) {
    expect(result.exit).not.toBe(0)
    expect(result.contents).toBe('')
  } else {
    expect(result.exit).toBe(0)
    expect(result.contents).toBe(
      `already_published=${String(scenario.published)}\n`,
    )
  }
  expect(result.stderr).not.toContain('fixture-token')
})

it('a missing credential blocks preparation before any request', async () => {
  const result = await runGuard(scenarios[0], undefined)
  expect(result.exit).not.toBe(0)
  expect(result.requests).toEqual([])
  expect(result.contents).toBe('')
})
