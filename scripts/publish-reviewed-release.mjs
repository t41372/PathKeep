/**
 * Verify or publish exactly the reviewed draft under the workflow's tag lock.
 * Responsibilities: bind approval to source and manifest, compare every current
 * asset, and promote without building. The repository's immutable-release setting
 * is an administrator preflight; the workflow token has no administration access.
 * Not responsible for choosing approval inputs, enabling that setting or uploads.
 */
import { createHash } from 'node:crypto'
import {
  checkTagSource,
  releaseApi,
  requireSourceSha,
} from './lib/release-api.mjs'

const source = requireSourceSha(process.env.SOURCE_COMMIT)
const approved = requireSourceSha(process.env.APPROVED_SOURCE_SHA)
const digest = process.env.APPROVED_MANIFEST_SHA256
const tag = process.env.RESOLVED_TAG
const id = process.env.RELEASE_ID
const execute = process.env.EXECUTE_PUBLICATION === 'true'
if (execute && process.env.IMMUTABLE_RELEASES_CONFIRMED !== 'true') {
  throw new Error(
    'Confirm the administrator checked immutable releases before publication.',
  )
}
if (
  source !== approved ||
  !/^[a-f0-9]{64}$/.test(digest ?? '') ||
  !tag ||
  !/^\d+$/.test(id ?? '')
) {
  throw new Error(
    'Approved source, release ID, tag and manifest digest are required.',
  )
}
if (
  execute &&
  (process.env.GITHUB_ACTIONS !== 'true' ||
    process.env.GITHUB_JOB !== 'publish-release')
) {
  throw new Error(
    'Publish through the release workflow to retain its tag lock.',
  )
}
const api = releaseApi()
const release = await api.json(`/releases/${id}`)
if (
  release.id !== Number(id) ||
  release.tag_name !== tag ||
  release.target_commitish !== source ||
  typeof release.draft !== 'boolean' ||
  !Array.isArray(release.assets)
) {
  throw new Error('Release metadata does not match the approved source.')
}
await checkTagSource(api, tag, source)
const assets = new Map(release.assets.map((asset) => [asset.name, asset]))
if (assets.size !== release.assets.length)
  throw new Error('Duplicate release assets.')
const manifestAsset = assets.get('RELEASE-MANIFEST.json')
const checksumAsset = assets.get('SHA256SUMS.txt')
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex')
if (manifestAsset?.digest !== `sha256:${digest}` || !checksumAsset) {
  throw new Error('The approved manifest changed or checksums are missing.')
}
const manifestBytes = await api.controlFile(manifestAsset)
if (sha256(manifestBytes) !== digest)
  throw new Error('Manifest download differs from approval.')
const manifest = JSON.parse(manifestBytes.toString())
if (
  manifest.releaseTag !== tag ||
  manifest.sourceCommit !== source ||
  !Array.isArray(manifest.files) ||
  manifest.files.length === 0 ||
  assets.size !== manifest.files.length + 2
) {
  throw new Error('Manifest does not cover the approved release.')
}
const expected = new Map()
for (const file of manifest.files) {
  const actual = assets.get(file.name)
  if (
    typeof file.name !== 'string' ||
    /[\r\n/\\]/.test(file.name) ||
    ['SHA256SUMS.txt', 'RELEASE-MANIFEST.json'].includes(file.name) ||
    expected.has(file.name) ||
    !/^[a-f0-9]{64}$/.test(file.sha256) ||
    !Number.isSafeInteger(file.sizeBytes) ||
    file.sizeBytes < 1 ||
    actual?.digest !== `sha256:${file.sha256}` ||
    actual.size !== file.sizeBytes
  ) {
    throw new Error('Release assets differ from the reviewed manifest.')
  }
  expected.set(file.name, file.sha256)
}
const checksumBytes = await api.controlFile(checksumAsset)
if (checksumAsset.digest !== `sha256:${sha256(checksumBytes)}`)
  throw new Error('Checksum download changed.')
const listed = new Map()
for (const line of checksumBytes.toString().trim().split('\n')) {
  const match = /^([a-f0-9]{64})\s+\*?(?:\.\/)?(.+)$/.exec(line)
  if (!match || listed.has(match[2])) throw new Error('Invalid checksum list.')
  listed.set(match[2], match[1])
}
if (
  listed.size !== expected.size ||
  [...expected].some(([name, hash]) => listed.get(name) !== hash)
) {
  throw new Error('Checksums disagree with the reviewed manifest.')
}
if (!release.draft && release.immutable !== true)
  throw new Error('Published release is not immutable.')
if (execute && release.draft) {
  const published = await api.json(`/releases/${id}`, {
    method: 'PATCH',
    body: {
      draft: false,
      make_latest: release.prerelease ? 'false' : 'true',
    },
  })
  if (published.draft !== false || published.immutable !== true) {
    throw new Error(
      'Publication did not confirm server-side immutability. Do not upload replacements.',
    )
  }
}
console.log(
  JSON.stringify({
    releaseId: Number(id),
    tag,
    sourceCommit: source,
    manifestSha256: digest,
    assets: assets.size,
    publicationRequested: execute,
    alreadyPublished: !release.draft,
  }),
)
