/**
 * Inspect publication state before preparation or an upload attempt.
 * Responsibilities: skip public tags and verify a reserved draft's source.
 * Not responsible for atomic protection: the workflow tag lock serializes its
 * operations, and GitHub immutable releases protect published assets server-side.
 */
import {
  releaseApi,
  releaseOutputs,
  requireSourceSha,
} from './lib/release-api.mjs'

const tag = process.env.RESOLVED_TAG
const id = process.env.RELEASE_ID
if (!tag) throw new Error('Release tag is required.')
const api = releaseApi()
let release
if (id) {
  if (!/^\d+$/.test(id)) throw new Error('Invalid release ID.')
  const source = requireSourceSha(process.env.SOURCE_COMMIT)
  release = await api.json(`/releases/${id}`)
  if (release.id !== Number(id) || release.target_commitish !== source) {
    throw new Error('Reserved release source changed.')
  }
} else {
  release = await api.json(`/releases/tags/${encodeURIComponent(tag)}`, {
    notFound: true,
  })
}
if (
  release !== null &&
  (release === undefined ||
    typeof release !== 'object' ||
    release.tag_name !== tag ||
    typeof release.draft !== 'boolean')
) {
  throw new Error('Release metadata did not match the requested tag and state.')
}
const published = release !== null && !release.draft
releaseOutputs({ already_published: published })
console.log(
  published
    ? 'Published release: skip builds and asset uploads.'
    : 'No published release: continue preparation.',
)
