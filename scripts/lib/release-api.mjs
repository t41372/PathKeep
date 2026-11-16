/**
 * Shared transport for release metadata and small control files.
 * Responsibilities: authenticated bounded requests, tag ownership and Actions
 * output encoding. Not responsible for deciding when publication is approved.
 */
import { appendFileSync } from 'node:fs'

/** Reject ambiguous source identifiers before using them in release decisions. */
export function requireSourceSha(value) {
  if (!/^(?:[a-f0-9]{40}|[a-f0-9]{64})$/.test(value ?? '')) {
    throw new Error('A full source commit SHA is required.')
  }
  return value
}

/** Keep every release command on the same authenticated API boundary. */
export function releaseApi() {
  const repository = process.env.GITHUB_REPOSITORY
  const token = process.env.GITHUB_TOKEN
  const base = (process.env.GITHUB_API_URL ?? 'https://api.github.com').replace(
    /\/$/,
    '',
  )
  if (!repository || !token)
    throw new Error('Repository and token are required.')
  const prefix = `${base}/repos/${repository}`
  const headers = {
    Authorization: `Bearer ${token}`,
    Accept: 'application/vnd.github+json',
    'X-GitHub-Api-Version': '2026-03-10',
  }

  async function json(route, { method = 'GET', body, notFound = false } = {}) {
    const response = await fetch(prefix + route, {
      method,
      headers: { ...headers, 'content-type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
      redirect: 'error',
      signal: AbortSignal.timeout(30_000),
    })
    if (notFound && response.status === 404) return null
    if (!response.ok)
      throw new Error(`GitHub ${method} ${route} failed (${response.status}).`)
    try {
      return await response.json()
    } catch {
      throw new Error('GitHub returned invalid JSON.')
    }
  }

  async function controlFile(asset) {
    const limit = 1024 * 1024
    if (!Number.isSafeInteger(asset.id) || asset.id < 1 || asset.size > limit) {
      throw new Error('Invalid release control file metadata.')
    }
    const response = await fetch(`${prefix}/releases/assets/${asset.id}`, {
      headers: { ...headers, Accept: 'application/octet-stream' },
      signal: AbortSignal.timeout(30_000),
    })
    if (!response.ok || !response.body)
      throw new Error('Release control file download failed.')
    const chunks = []
    let size = 0
    for await (const chunk of response.body) {
      size += chunk.length
      if (size > limit)
        throw new Error('Release control file exceeds its limit.')
      chunks.push(chunk)
    }
    if (size !== asset.size)
      throw new Error('Release control file is truncated.')
    return Buffer.concat(chunks)
  }

  return { json, controlFile }
}

/** Refuse an existing tag that names another source, including annotated tags. */
export async function checkTagSource(api, tag, source) {
  const ref = await api.json(`/git/ref/tags/${encodeURIComponent(tag)}`, {
    notFound: true,
  })
  if (ref === null) return
  let object = ref.object
  const visited = new Set()
  while (object?.type === 'tag') {
    requireSourceSha(object.sha)
    if (visited.has(object.sha)) throw new Error('Cyclic tag metadata.')
    visited.add(object.sha)
    object = (await api.json(`/git/tags/${object.sha}`)).object
  }
  if (object?.type !== 'commit' || object.sha !== source) {
    throw new Error('The release tag points to another source commit.')
  }
}

/** Actions outputs must remain single values, never injected output lines. */
export function releaseOutputs(values) {
  const output = process.env.GITHUB_OUTPUT
  if (!output) throw new Error('GitHub Actions output file is required.')
  for (const [key, value] of Object.entries(values)) {
    if (!/^\w+$/.test(key) || /[\r\n]/.test(String(value)))
      throw new Error('Invalid Actions output.')
    appendFileSync(output, `${key}=${String(value)}\n`)
  }
}
