/**
 * Guards draft ownership and publication of the exact reviewed bytes.
 * Responsibilities: exercise the real CLIs against controlled HTTP responses.
 * Not responsible for building packages; the real Actions rehearsal covers job
 * routing/concurrency, and the application gate covers the native backend.
 *
 * Failure modes written before implementation:
 * - a queued ref reuses another commit's private draft or existing tag;
 * - a publication uses a different source, manifest, missing or replaced asset;
 * - checksum text disagrees with the reviewed manifest;
 * - preview writes, or a local invocation bypasses the workflow's tag lock;
 * - retrying a completed immutable publication edits it again;
 * - a prerelease changes the current stable latest release;
 * - a resumed draft silently changes release channel;
 * - a metadata PATCH loses its approved tag, or its response goes unchecked;
 * - an Actions token tries to create/publish a source with unapproved workflow changes;
 * - API failures silently allow mutation.
 */
import { createHash } from 'node:crypto'
import { expect, it } from 'vitest'
import {
  releaseTag,
  runReleaseCli,
  sourceSha,
  type ReleaseRequest,
} from './test-support/release-cli'

const sha256 = (text: string) => createHash('sha256').update(text).digest('hex')
const packageName = 'PathKeep_0.5.0_fixture.pkg'
const packageBytes = 'native package fixture'
const packageHash = sha256(packageBytes)
const checksums = `${packageHash}  ./${packageName}\n`
const manifest = JSON.stringify({
  releaseTag,
  sourceCommit: sourceSha,
  files: [
    {
      name: packageName,
      sizeBytes: Buffer.byteLength(packageBytes),
      sha256: packageHash,
    },
  ],
})
const assets = [
  {
    id: 101,
    name: packageName,
    size: Buffer.byteLength(packageBytes),
    digest: `sha256:${packageHash}`,
  },
  {
    id: 102,
    name: 'SHA256SUMS.txt',
    size: Buffer.byteLength(checksums),
    digest: `sha256:${sha256(checksums)}`,
  },
  {
    id: 103,
    name: 'RELEASE-MANIFEST.json',
    size: Buffer.byteLength(manifest),
    digest: `sha256:${sha256(manifest)}`,
  },
]

function draft(overrides: Record<string, unknown> = {}) {
  return {
    id: 41,
    tag_name: releaseTag,
    target_commitish: sourceSha,
    draft: true,
    prerelease: false,
    immutable: false,
    assets,
    ...overrides,
  }
}

function workflowApi(request: ReleaseRequest, sameWorkflows: boolean) {
  if (request.path === '/repos/owner/repo')
    return { body: { default_branch: 'main' } }
  const github = 'd'.repeat(40)
  const defaultGithub = 'f'.repeat(40)
  const workflows = 'e'.repeat(40)
  const tree = (path: string, sha: string) => ({
    body: { truncated: false, tree: [{ path, type: 'tree', sha }] },
  })
  if (request.path.endsWith(`/git/trees/${sourceSha}`))
    return tree('.github', github)
  if (request.path.endsWith('/git/trees/main'))
    return tree('.github', defaultGithub)
  if (request.path.endsWith(`/git/trees/${github}`))
    return tree('workflows', workflows)
  if (request.path.endsWith(`/git/trees/${defaultGithub}`))
    return tree('workflows', sameWorkflows ? workflows : 'b'.repeat(40))
  return undefined
}

function publicationApi(
  release = draft(),
  checksumText = checksums,
  options: {
    sameWorkflows?: boolean
    published?: Record<string, unknown>
  } = {},
) {
  return (request: ReleaseRequest) => {
    const workflow = workflowApi(request, options.sameWorkflows ?? true)
    if (workflow) return workflow
    if (request.path.endsWith('/git/ref/tags/v0.5.0')) {
      return { status: 404 }
    }
    if (request.path.endsWith('/releases/assets/103')) return { raw: manifest }
    if (request.path.endsWith('/releases/assets/102')) {
      return { raw: checksumText }
    }
    if (request.path.endsWith('/releases/41')) {
      return {
        body:
          request.method === 'PATCH'
            ? {
                ...release,
                // The real API clears tag association when this field is omitted.
                tag_name:
                  (request.body as Record<string, unknown>)?.tag_name ??
                  'untagged-fixture',
                draft: false,
                immutable: true,
                ...options.published,
              }
            : release,
      }
    }
    return { status: 404 }
  }
}

const approval = {
  APPROVED_SOURCE_SHA: sourceSha,
  APPROVED_MANIFEST_SHA256: sha256(manifest),
}

it('preview validates the reviewed draft without publishing it', async () => {
  const result = await runReleaseCli(
    'publish-reviewed-release.mjs',
    publicationApi(),
    approval,
  )
  expect(result.exit).toBe(0)
  expect(result.requests.some((r) => r.path.endsWith('/releases/41'))).toBe(
    true,
  )
  expect(result.requests.every((r) => r.method === 'GET')).toBe(true)
})

it('preview reports workflow approval still pending without writing', async () => {
  const result = await runReleaseCli(
    'publish-reviewed-release.mjs',
    publicationApi(draft(), checksums, { sameWorkflows: false }),
    approval,
  )
  expect(result.exit).toBe(0)
  expect(JSON.parse(result.stdout).workflowsOnDefaultBranch).toBe(false)
  expect(result.requests.every((r) => r.method === 'GET')).toBe(true)
})

it('publication changes only the validated draft under the workflow lock', async () => {
  const result = await runReleaseCli(
    'publish-reviewed-release.mjs',
    publicationApi(),
    { ...approval, EXECUTE_PUBLICATION: 'true' },
  )
  expect(result.exit).toBe(0)
  expect(result.requests.filter((r) => r.method !== 'GET')).toEqual([
    {
      method: 'PATCH',
      path: '/repos/owner/repo/releases/41',
      body: {
        tag_name: releaseTag,
        draft: false,
        prerelease: false,
        make_latest: 'true',
      },
      authorization: 'Bearer fixture-token',
    },
  ])
  expect(result.stderr).not.toContain('fixture-token')
})

it('publishing a prerelease preserves the current stable latest release', async () => {
  const result = await runReleaseCli(
    'publish-reviewed-release.mjs',
    publicationApi(draft({ prerelease: true })),
    { ...approval, EXECUTE_PUBLICATION: 'true', PRERELEASE: 'true' },
  )
  expect(result.exit).toBe(0)
  expect(result.requests.filter((r) => r.method === 'PATCH')).toEqual([
    {
      method: 'PATCH',
      path: '/repos/owner/repo/releases/41',
      body: {
        tag_name: releaseTag,
        draft: false,
        prerelease: true,
        make_latest: 'false',
      },
      authorization: 'Bearer fixture-token',
    },
  ])
})

it('publication refuses workflow changes not yet on the default branch', async () => {
  const result = await runReleaseCli(
    'publish-reviewed-release.mjs',
    publicationApi(draft(), checksums, { sameWorkflows: false }),
    { ...approval, EXECUTE_PUBLICATION: 'true' },
  )
  expect(result.exit).not.toBe(0)
  expect(result.requests.every((r) => r.method === 'GET')).toBe(true)
})

it('publication detects a response that lost the approved tag', async () => {
  const result = await runReleaseCli(
    'publish-reviewed-release.mjs',
    publicationApi(draft(), checksums, {
      published: { tag_name: 'untagged-unexpected' },
    }),
    { ...approval, EXECUTE_PUBLICATION: 'true' },
  )
  expect(result.exit).not.toBe(0)
  expect(result.requests.filter((r) => r.method === 'PATCH')).toHaveLength(1)
})

it.each([
  [false, 'true'],
  [true, 'false'],
] as const)(
  'publication refuses stored prerelease %s when requested %s',
  async (stored, requested) => {
    const result = await runReleaseCli(
      'publish-reviewed-release.mjs',
      publicationApi(draft({ prerelease: stored })),
      { ...approval, EXECUTE_PUBLICATION: 'true', PRERELEASE: requested },
    )
    expect(result.exit).not.toBe(0)
    expect(result.requests.every((r) => r.method === 'GET')).toBe(true)
  },
)

it.each([
  ['a different source', draft({ target_commitish: 'b'.repeat(40) })],
  ['a different tag', draft({ tag_name: 'v0.6.0' })],
  ['a missing asset', draft({ assets: assets.slice(1) })],
  [
    'replaced bytes',
    draft({
      assets: assets.map((a) =>
        a.id === 101 ? { ...a, digest: `sha256:${'b'.repeat(64)}` } : a,
      ),
    }),
  ],
  ['a mutable published release', draft({ draft: false, immutable: false })],
] as const)(
  'publication refuses %s without writing',
  async (_name, release) => {
    const result = await runReleaseCli(
      'publish-reviewed-release.mjs',
      publicationApi(release),
      { ...approval, EXECUTE_PUBLICATION: 'true' },
    )
    expect(result.exit).not.toBe(0)
    expect(result.requests.length).toBeGreaterThan(0)
    expect(result.requests.every((r) => r.method === 'GET')).toBe(true)
  },
)

it('publication refuses a changed approval digest', async () => {
  const result = await runReleaseCli(
    'publish-reviewed-release.mjs',
    publicationApi(),
    {
      ...approval,
      APPROVED_MANIFEST_SHA256: 'b'.repeat(64),
      EXECUTE_PUBLICATION: 'true',
    },
  )
  expect(result.exit).not.toBe(0)
  expect(result.requests.every((r) => r.method === 'GET')).toBe(true)
})

it('publication refuses checksum text that disagrees with the manifest', async () => {
  const text = `${'b'.repeat(64)}  ./${packageName}\n`
  const release = draft({
    assets: assets.map((a) =>
      a.id === 102
        ? {
            ...a,
            size: Buffer.byteLength(text),
            digest: `sha256:${sha256(text)}`,
          }
        : a,
    ),
  })
  const result = await runReleaseCli(
    'publish-reviewed-release.mjs',
    publicationApi(release, text),
    { ...approval, EXECUTE_PUBLICATION: 'true' },
  )
  expect(result.exit).not.toBe(0)
  expect(result.requests.every((r) => r.method === 'GET')).toBe(true)
})

it('a local execute invocation cannot bypass the workflow tag lock', async () => {
  const result = await runReleaseCli(
    'publish-reviewed-release.mjs',
    publicationApi(),
    { ...approval, EXECUTE_PUBLICATION: 'true', GITHUB_ACTIONS: undefined },
  )
  expect(result.exit).not.toBe(0)
  expect(result.requests).toEqual([])
})

it('publication requires the administrator immutability preflight confirmation', async () => {
  const result = await runReleaseCli(
    'publish-reviewed-release.mjs',
    publicationApi(),
    {
      ...approval,
      EXECUTE_PUBLICATION: 'true',
      IMMUTABLE_RELEASES_CONFIRMED: 'false',
    },
  )
  expect(result.exit).not.toBe(0)
  expect(result.requests.every((r) => r.method === 'GET')).toBe(true)
})

it('retrying the same immutable publication writes nothing', async () => {
  const result = await runReleaseCli(
    'publish-reviewed-release.mjs',
    publicationApi(draft({ draft: false, immutable: true })),
    { ...approval, EXECUTE_PUBLICATION: 'true' },
  )
  expect(result.exit).toBe(0)
  expect(result.requests.every((r) => r.method === 'GET')).toBe(true)
})

it('an API failure never publishes', async () => {
  const result = await runReleaseCli(
    'publish-reviewed-release.mjs',
    () => ({ status: 503 }),
    { ...approval, EXECUTE_PUBLICATION: 'true' },
  )
  expect(result.exit).not.toBe(0)
  expect(result.requests.every((r) => r.method === 'GET')).toBe(true)
})

function reservationApi(existing: unknown[] = [], sameWorkflows = true) {
  return (request: ReleaseRequest) => {
    const workflow = workflowApi(request, sameWorkflows)
    if (workflow) return workflow
    if (request.path.includes('/git/ref/tags/')) return { status: 404 }
    if (request.method === 'POST') return { body: draft() }
    return { body: existing }
  }
}

it('a fresh upload guard reads the reserved release and skips it after publication', async () => {
  const result = await runReleaseCli(
    'guard-published-release.mjs',
    publicationApi(draft({ draft: false, immutable: true })),
  )
  expect(result.exit).toBe(0)
  expect(result.requests[0].path).toBe('/repos/owner/repo/releases/41')
  expect(result.contents).toBe('already_published=true\n')
})

it('a fresh upload guard rejects a draft whose source changed', async () => {
  const result = await runReleaseCli(
    'guard-published-release.mjs',
    publicationApi(draft({ target_commitish: 'b'.repeat(40) })),
  )
  expect(result.exit).not.toBe(0)
  expect(result.contents).toBe('')
})

it('a new draft is private and bound to the checked-out source', async () => {
  const result = await runReleaseCli(
    'prepare-release-draft.mjs',
    reservationApi(),
  )
  expect(result.exit).toBe(0)
  const creates = result.requests.filter((r) => r.method === 'POST')
  expect(creates).toHaveLength(1)
  expect(creates[0].body).toMatchObject({
    tag_name: releaseTag,
    target_commitish: sourceSha,
    draft: true,
  })
  expect(result.contents).toBe('release_id=41\n')
})

it('the same source can resume its own draft without recreating it', async () => {
  const result = await runReleaseCli(
    'prepare-release-draft.mjs',
    reservationApi([draft()]),
  )
  expect(result.exit).toBe(0)
  expect(result.requests.every((r) => r.method === 'GET')).toBe(true)
  expect(result.contents).toBe('release_id=41\n')
})

it.each([
  [false, 'true'],
  [true, 'false'],
] as const)(
  'preparation refuses a stored prerelease flag %s when requested %s',
  async (stored, requested) => {
    const result = await runReleaseCli(
      'prepare-release-draft.mjs',
      reservationApi([draft({ prerelease: stored })]),
      { PRERELEASE: requested },
    )
    expect(result.exit).not.toBe(0)
    expect(result.requests.every((r) => r.method === 'GET')).toBe(true)
    expect(result.contents).toBe('')
  },
)

it('an invalid prerelease input is rejected before resuming a draft', async () => {
  const result = await runReleaseCli(
    'prepare-release-draft.mjs',
    reservationApi([draft()]),
    { PRERELEASE: 'invalid' },
  )
  expect(result.exit).not.toBe(0)
  expect(result.requests).toEqual([])
})

it('an Actions token leaves workflow-changing draft creation to the owner', async () => {
  const result = await runReleaseCli(
    'prepare-release-draft.mjs',
    reservationApi([], false),
  )
  expect(result.exit).not.toBe(0)
  expect(result.requests.every((r) => r.method === 'GET')).toBe(true)
  expect(result.contents).toBe('')
})

it('the owner can reserve a private workflow-changing draft outside Actions', async () => {
  const result = await runReleaseCli(
    'prepare-release-draft.mjs',
    reservationApi([], false),
    { GITHUB_ACTIONS: undefined },
  )
  expect(result.exit).toBe(0)
  expect(result.requests.filter((r) => r.method === 'POST')).toHaveLength(1)
})

it.each([
  ['another source draft', draft({ target_commitish: 'b'.repeat(40) })],
  ['a published release', draft({ draft: false })],
] as const)('preparation refuses %s', async (_name, release) => {
  const result = await runReleaseCli(
    'prepare-release-draft.mjs',
    reservationApi([release]),
  )
  expect(result.exit).not.toBe(0)
  expect(result.requests.length).toBeGreaterThan(0)
  expect(result.requests.every((r) => r.method === 'GET')).toBe(true)
})

it('a different existing tag commit blocks draft creation', async () => {
  const result = await runReleaseCli('prepare-release-draft.mjs', (request) =>
    request.path.includes('/git/ref/')
      ? { body: { object: { type: 'commit', sha: 'b'.repeat(40) } } }
      : { body: [] },
  )
  expect(result.exit).not.toBe(0)
  expect(result.requests.every((r) => r.method === 'GET')).toBe(true)
})

it('an annotated tag is checked against its underlying commit', async () => {
  const result = await runReleaseCli('prepare-release-draft.mjs', (request) => {
    if (request.path.includes('/git/ref/')) {
      return { body: { object: { type: 'tag', sha: 'b'.repeat(40) } } }
    }
    if (request.path.includes('/git/tags/')) {
      return { body: { object: { type: 'commit', sha: sourceSha } } }
    }
    return reservationApi()(request)
  })
  expect(result.exit).toBe(0)
  expect(result.requests.filter((r) => r.method === 'POST')).toHaveLength(1)
})
