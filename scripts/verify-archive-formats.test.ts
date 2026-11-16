// @vitest-environment node
/**
 * The check that keeps released versions and archive format revisions in step. Written before
 * the check, from the ways it could let a bad release through:
 *
 * 1. A newer release writes a lower revision than an older one: users who update are locked out.
 * 2. Versions compared as text: 0.10.0 sorted before 0.9.0, or a prerelease after its release.
 * 3. A release names a revision that has no entry under `formats`.
 * 4. A release writes a revision that is not frozen under `released`.
 * 5. The build writes a lower revision than a version already released.
 * 6. Releasing a version that is not listed, or listed with another revision than the build's.
 * 7. Releasing while a format change is half done (the persisted shapes a revision ahead).
 * 8. Releasing a revision whose entry still says it is in development.
 * 9. The same version listed twice.
 * 10. The repository as it stands passes, and releasing 0.6.0 fails until it is recorded.
 */
import { describe, expect, it } from 'vitest'
import { spawnSync } from 'node:child_process'

interface Formats {
  formats: { revision: number; status: string }[]
  released: { rootRevision: number; schemaSha256: string }[]
  releases: { appVersion: string; rootRevision: number }[]
}
interface Input {
  formats: Formats
  level: number
  revision: number
  releasing?: string
}
interface FormatsModule {
  checkArchiveFormats: (input: Input) => string[]
}

const { checkArchiveFormats } =
  // @ts-expect-error plain ESM script under test does not ship a declaration file
  (await import('./verify-archive-formats.mjs')) as FormatsModule

/** Revision 0 released by 0.4.0 and 0.5.0, revision 1 frozen and released by 0.6.0. */
function shipped(): Formats {
  return {
    formats: [
      { revision: 0, status: 'released' },
      { revision: 1, status: 'released' },
    ],
    released: [{ rootRevision: 1, schemaSha256: 'ab' }],
    releases: [
      { appVersion: '0.4.0', rootRevision: 0 },
      { appVersion: '0.5.0', rootRevision: 0 },
      { appVersion: '0.6.0', rootRevision: 1 },
    ],
  }
}

describe('released versions and archive revisions', () => {
  it('passes a history where no newer release writes less', () => {
    expect(
      checkArchiveFormats({ formats: shipped(), level: 1, revision: 1 }),
    ).toEqual([])
    expect(
      checkArchiveFormats({
        formats: shipped(),
        level: 1,
        revision: 1,
        releasing: '0.6.0',
      }),
    ).toEqual([])
  })

  it('refuses a newer release that writes a lower revision', () => {
    const formats = shipped()
    formats.releases.push({ appVersion: '0.6.1', rootRevision: 0 })
    expect(
      checkArchiveFormats({ formats, level: 1, revision: 1 }).join('\n'),
    ).toMatch(/0\.6\.1 writes revision 0, lower than 0\.6\.0/)
  })

  it('orders versions by number, not by text', () => {
    const formats = shipped()
    // As text, 0.10.0 sorts before 0.9.0 and the lower revision would look older.
    formats.releases.push(
      { appVersion: '0.9.0', rootRevision: 1 },
      { appVersion: '0.10.0', rootRevision: 0 },
    )
    expect(
      checkArchiveFormats({ formats, level: 1, revision: 1 }).join('\n'),
    ).toMatch(/0\.10\.0 writes revision 0, lower than 0\.9\.0/)
  })

  it('puts a prerelease before its release', () => {
    const formats = shipped()
    formats.releases = formats.releases.filter((r) => r.appVersion !== '0.6.0')
    formats.releases.push(
      { appVersion: '0.6.0', rootRevision: 0 },
      { appVersion: '0.6.0-beta.2', rootRevision: 1 },
      { appVersion: '0.6.0-beta.10', rootRevision: 1 },
    )
    const failures = checkArchiveFormats({
      formats,
      level: 1,
      revision: 1,
    }).join('\n')
    expect(failures).toMatch(
      /0\.6\.0 writes revision 0, lower than 0\.6\.0-beta\.10/,
    )
    // beta.10 comes after beta.2: numeric identifiers compare as numbers.
    expect(failures).not.toMatch(/beta\.2 writes revision/)
  })

  it('refuses a release of a revision with no entry under formats', () => {
    const formats = shipped()
    formats.releases.push({ appVersion: '0.7.0', rootRevision: 2 })
    expect(
      checkArchiveFormats({ formats, level: 2, revision: 2 }).join('\n'),
    ).toMatch(/0\.7\.0 writes revision 2, which has no entry under formats/)
  })

  it('refuses a release of a revision that is not frozen', () => {
    const formats = shipped()
    formats.released = []
    expect(
      checkArchiveFormats({ formats, level: 1, revision: 1 }).join('\n'),
    ).toMatch(/0\.6\.0 writes revision 1, which is not listed under released/)
  })

  it('refuses a build that writes less than a released version', () => {
    expect(
      checkArchiveFormats({ formats: shipped(), level: 0, revision: 0 }).join(
        '\n',
      ),
    ).toMatch(/this build writes revision 0, lower than 0\.6\.0/)
  })

  it('refuses to release a version that is not listed, or listed otherwise', () => {
    expect(
      checkArchiveFormats({
        formats: shipped(),
        level: 1,
        revision: 1,
        releasing: '0.6.1',
      }).join('\n'),
    ).toMatch(/0\.6\.1 is not listed under releases/)
    const formats = shipped()
    formats.formats.push({ revision: 2, status: 'released' })
    formats.released.push({ rootRevision: 2, schemaSha256: 'cd' })
    expect(
      checkArchiveFormats({
        formats,
        level: 2,
        revision: 2,
        releasing: '0.6.0',
      }).join('\n'),
    ).toMatch(
      /0\.6\.0 is listed as writing revision 1, but this build writes revision 2/,
    )
  })

  it('refuses to release in the middle of a format change', () => {
    expect(
      checkArchiveFormats({
        formats: shipped(),
        level: 1,
        revision: 2,
        releasing: '0.6.0',
      }).join('\n'),
    ).toMatch(
      /persisted shapes are revision 2's while this build writes revision 1/,
    )
  })

  it('refuses to release a revision still in development', () => {
    const formats = shipped()
    formats.formats[1].status = 'in development'
    expect(
      checkArchiveFormats({
        formats,
        level: 1,
        revision: 1,
        releasing: '0.6.0',
      }).join('\n'),
    ).toMatch(/revision 1's entry under formats says "in development"/)
  })

  it('refuses a version listed twice', () => {
    const formats = shipped()
    formats.releases.push({ appVersion: '0.5.0', rootRevision: 0 })
    expect(
      checkArchiveFormats({ formats, level: 1, revision: 1 }).join('\n'),
    ).toMatch(/0\.5\.0 is listed twice/)
  })

  it('passes the repository, and refuses to release 0.6.0 until it is recorded', () => {
    const run = (...args: string[]) =>
      spawnSync('node', ['scripts/verify-archive-formats.mjs', ...args], {
        encoding: 'utf8',
      })
    const now = run()
    expect(now.stderr).toBe('')
    expect(now.status).toBe(0)
    const release = run('--releasing', '0.6.0')
    expect(release.status).toBe(1)
    expect(release.stderr).toMatch(/0\.6\.0 is not listed under releases/)
  })
})
