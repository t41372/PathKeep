/**
 * Keep already published binaries immutable when a release workflow repeats.
 * Responsibilities: look up the resolved tag and report whether build/upload
 * jobs must be skipped. API or metadata failures stop preparation.
 * Not responsible for: resolving versions, building bundles or publication.
 * Runs in GitHub Actions with its token; uses Node built-ins only.
 */
import { appendFileSync } from 'node:fs'

const {
  GITHUB_API_URL = 'https://api.github.com',
  GITHUB_REPOSITORY,
  GITHUB_TOKEN,
  GITHUB_OUTPUT,
  RESOLVED_TAG,
} = process.env

if (!GITHUB_REPOSITORY || !GITHUB_TOKEN || !GITHUB_OUTPUT || !RESOLVED_TAG) {
  throw new Error(
    'The release guard requires repository, token, tag and output.',
  )
}

const endpoint = `${GITHUB_API_URL.replace(/\/$/, '')}/repos/${GITHUB_REPOSITORY}/releases/tags/${encodeURIComponent(RESOLVED_TAG)}`
const response = await fetch(endpoint, {
  headers: {
    Accept: 'application/vnd.github+json',
    Authorization: `Bearer ${GITHUB_TOKEN}`,
  },
  redirect: 'error',
  signal: AbortSignal.timeout(30_000),
})

let published = false
if (response.status !== 404) {
  if (!response.ok) {
    throw new Error(`GitHub release lookup failed (${response.status}).`)
  }
  const release = await response.json()
  if (
    release === null ||
    typeof release !== 'object' ||
    release.tag_name !== RESOLVED_TAG ||
    typeof release.draft !== 'boolean'
  ) {
    throw new Error(
      'Release metadata did not match the requested tag and state.',
    )
  }
  published = !release.draft
}

appendFileSync(GITHUB_OUTPUT, `already_published=${String(published)}\n`)
console.log(
  published
    ? 'Published release: skip builds and asset uploads.'
    : 'No published release: continue preparation.',
)
