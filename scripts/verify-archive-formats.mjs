/**
 * Keeps released PathKeep versions and archive format revisions in step.
 *
 * ## Responsibilities
 * - Fail when a newer release writes a lower archive revision than an older one: users who
 *   update would find the archive the older release wrote refused as too new.
 * - Fail when a released version writes a revision that is not frozen (`released` in
 *   `formats.json`), so its shapes could still change after users have archives in it.
 * - Fail when this build writes a lower revision than a version already released.
 * - With `--releasing <version>`: fail unless that version is listed, writes the revision this
 *   build writes, and that revision is frozen and described as released, with no format change
 *   half done.
 *
 * ## Not responsible for
 * - Comparing a frozen revision's snapshot with its hash
 *   (`vault-core/tests/persisted_schema.rs`).
 * - Deciding what an older build does with a newer archive: a person writes that in
 *   `formats.json`, next to the revision.
 *
 * ## Dependencies
 * - Node.js built-ins only. Reads `formats.json` and two constants from the Rust source.
 */
import { readFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'

const FORMATS =
  'src-tauri/crates/vault-core/tests/persisted_schema/formats.json'

/** `major.minor.patch`, with prerelease identifiers; build metadata is ignored. */
function parseVersion(text) {
  const match =
    /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/.exec(
      String(text),
    )
  if (!match) return null
  return {
    core: [Number(match[1]), Number(match[2]), Number(match[3])],
    pre: match[4] ? match[4].split('.') : [],
  }
}

/** Semantic-version order: numbers compare as numbers, a prerelease comes before its release. */
function compareVersions(a, b) {
  for (let i = 0; i < 3; i++) {
    if (a.core[i] !== b.core[i]) return a.core[i] - b.core[i]
  }
  if (a.pre.length === 0 || b.pre.length === 0) {
    return b.pre.length - a.pre.length
  }
  for (let i = 0; i < Math.max(a.pre.length, b.pre.length); i++) {
    const x = a.pre[i]
    const y = b.pre[i]
    if (x === undefined) return -1
    if (y === undefined) return 1
    const xNumber = /^\d+$/.test(x)
    const yNumber = /^\d+$/.test(y)
    if (xNumber && yNumber) {
      if (Number(x) !== Number(y)) return Number(x) - Number(y)
    } else if (xNumber !== yNumber) {
      return xNumber ? -1 : 1
    } else if (x !== y) {
      return x < y ? -1 : 1
    }
  }
  return 0
}

/**
 * Every reason the format table, this build's revision (`level`) and the revision of its persisted
 * shapes (`revision`) do not fit together; with `releasing`, also every reason that version may not
 * be released from this build. Empty when they fit.
 */
export function checkArchiveFormats({ formats, level, revision, releasing }) {
  const failures = []
  const rows = new Map(
    (formats.formats ?? []).map((row) => [row.revision, row]),
  )
  const frozen = new Set(
    (formats.released ?? []).map((entry) => entry.rootRevision),
  )
  const listed = new Map()
  const ordered = []
  for (const entry of formats.releases ?? []) {
    const version = parseVersion(entry.appVersion)
    if (!version) {
      failures.push(
        `releases: ${JSON.stringify(entry.appVersion)} is not a version`,
      )
      continue
    }
    if (listed.has(entry.appVersion)) {
      failures.push(`releases: ${entry.appVersion} is listed twice`)
    }
    listed.set(entry.appVersion, entry)
    if (!rows.has(entry.rootRevision)) {
      failures.push(
        `releases: ${entry.appVersion} writes revision ${entry.rootRevision}, which has no entry under formats`,
      )
    } else if (entry.rootRevision > 0 && !frozen.has(entry.rootRevision)) {
      failures.push(
        `releases: ${entry.appVersion} writes revision ${entry.rootRevision}, which is not listed under released: its shapes could still change after users have archives in it`,
      )
    }
    ordered.push({ entry, version })
  }
  ordered.sort((a, b) => compareVersions(a.version, b.version))
  for (let i = 1; i < ordered.length; i++) {
    const older = ordered[i - 1].entry
    const newer = ordered[i].entry
    if (newer.rootRevision < older.rootRevision) {
      failures.push(
        `releases: ${newer.appVersion} writes revision ${newer.rootRevision}, lower than ${older.appVersion} (revision ${older.rootRevision}): users who update from ${older.appVersion} would find their archive refused as too new`,
      )
    }
  }
  const newest = ordered.reduce(
    (highest, { entry }) =>
      highest && highest.rootRevision >= entry.rootRevision ? highest : entry,
    null,
  )
  if (newest && level < newest.rootRevision) {
    failures.push(
      `this build writes revision ${level}, lower than ${newest.appVersion} (revision ${newest.rootRevision}), already released: its users would find their archive refused as too new`,
    )
  }
  if (!rows.has(level)) {
    failures.push(
      `this build writes revision ${level}, which has no entry under formats`,
    )
  }
  if (releasing !== undefined) {
    const entry = listed.get(releasing)
    if (!entry) {
      failures.push(
        `${releasing} is not listed under releases in formats.json: list it with the revision it writes (${level})`,
      )
    } else if (entry.rootRevision !== level) {
      failures.push(
        `${releasing} is listed as writing revision ${entry.rootRevision}, but this build writes revision ${level}`,
      )
    }
    if (revision !== level) {
      failures.push(
        `the persisted shapes are revision ${revision}'s while this build writes revision ${level}: a format change is half done`,
      )
    }
    if (level > 0 && !frozen.has(level)) {
      failures.push(
        `revision ${level} is not listed under released: freeze it, with the hash of its snapshot, before releasing`,
      )
    }
    const status = rows.get(level)?.status
    if (status !== 'released') {
      failures.push(
        `revision ${level}'s entry under formats says "${status}": describe what the release writes, as released`,
      )
    }
  }
  return failures
}

/** A `pub const NAME: u64 = N;` in a Rust source file. */
function rustConstant(file, name) {
  const match = new RegExp(`pub const ${name}: u64 = (\\d+);`).exec(
    readFileSync(file, 'utf8'),
  )
  if (!match) throw new Error(`${file} declares no ${name}`)
  return Number(match[1])
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const args = process.argv.slice(2)
  const at = args.indexOf('--releasing')
  const releasing = at >= 0 ? args[at + 1] : undefined
  if (at >= 0 && !releasing) {
    console.error(
      'Usage: node scripts/verify-archive-formats.mjs [--releasing <version>]',
    )
    process.exit(2)
  }
  const failures = checkArchiveFormats({
    formats: JSON.parse(readFileSync(FORMATS, 'utf8')),
    level: rustConstant(
      'src-tauri/crates/vault-core/src/admission.rs',
      'PRODUCT_ROOT_LEVEL',
    ),
    revision: rustConstant(
      'src-tauri/crates/vault-core/src/persisted_schema.rs',
      'REVISION',
    ),
    releasing,
  })
  if (failures.length > 0) {
    console.error(`${FORMATS}:\n- ${failures.join('\n- ')}`)
    process.exit(1)
  }
}
