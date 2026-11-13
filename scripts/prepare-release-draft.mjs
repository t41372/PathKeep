/**
 * Reserve a private release for one source while the workflow holds its tag lock.
 * Responsibilities: validate tag/draft ownership and create or resume that draft.
 * Not responsible for building, uploading or publishing assets.
 */
import {
  checkTagSource,
  releaseApi,
  releaseOutputs,
  requireSourceSha,
} from './lib/release-api.mjs'

const tag = process.env.RESOLVED_TAG
const source = requireSourceSha(process.env.SOURCE_COMMIT)
if (!tag) throw new Error('Release tag is required.')
const api = releaseApi()
await checkTagSource(api, tag, source)
const matches = []
for (let page = 1; ; page++) {
  const releases = await api.json(`/releases?per_page=100&page=${page}`)
  if (!Array.isArray(releases))
    throw new Error('Invalid release list metadata.')
  matches.push(...releases.filter((release) => release.tag_name === tag))
  if (releases.length < 100) break
}
if (matches.length > 1) throw new Error('Multiple releases use this tag.')
let release = matches[0]
if (release) {
  if (release.draft !== true || release.target_commitish !== source) {
    throw new Error('The release is published or belongs to another source.')
  }
} else {
  const prerelease = process.env.PRERELEASE ?? 'false'
  if (!['true', 'false'].includes(prerelease))
    throw new Error('Invalid prerelease flag.')
  release = await api.json('/releases', {
    method: 'POST',
    body: {
      tag_name: tag,
      target_commitish: source,
      name: `PathKeep ${tag}`,
      body: 'Private draft preparation. Publication requires review and approval.',
      draft: true,
      prerelease: prerelease === 'true',
    },
  })
}
if (
  !Number.isSafeInteger(release.id) ||
  release.id < 1 ||
  release.tag_name !== tag ||
  release.draft !== true ||
  release.target_commitish !== source
) {
  throw new Error('Draft metadata does not match the reserved source.')
}
releaseOutputs({ release_id: release.id })
console.log('Private draft reserved for the checked-out source.')
