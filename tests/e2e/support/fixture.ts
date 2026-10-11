/**
 * What the tests know about the synthetic browsers: every visit that was
 * written, per profile, and the files they live in. Expected numbers in the
 * specs are computed from this, never typed in by hand.
 *
 * Also a direct line to the backend, for checking side effects the UI does
 * not show (and for nothing else: user actions go through the UI).
 */
import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import { expect, type TestInfo } from '@playwright/test'
import type {
  SyntheticProfileId,
  SyntheticVisit,
} from '../../../scripts/fixtures/synthetic-browsers.mjs'
import { resolveDesktopBridgeEnv } from '../../../scripts/pathkeep-dev-desktop-bridge.mjs'

export type { SyntheticVisit as FixtureVisit } from '../../../scripts/fixtures/synthetic-browsers.mjs'
export type ProfileId = SyntheticProfileId

interface Fixture {
  root: string
  now: number
  seed: number
  visitsByProfile: Record<ProfileId, SyntheticVisit[]>
  files: Record<ProfileId, string>
  /** Folders the backend runs against; `osSandbox` stands in for launchd / Task Scheduler. */
  dirs: {
    projectRoot: string
    keyring: string
    noSafari: string
    osSandbox: string
  }
}

let cached: Fixture | null = null

export function fixture(): Fixture {
  if (cached) return cached
  const file = process.env.PATHKEEP_E2E_FIXTURE
  if (!file)
    throw new Error('PATHKEEP_E2E_FIXTURE is not set; run via the E2E config.')
  cached = JSON.parse(readFileSync(file, 'utf8')) as Fixture
  return cached
}

/** `YYYY-MM-DD` of `at` in this computer's time zone: History's days, and `after:`'s. */
export function localDay(at: number) {
  const day = new Date(at)
  const pad = (value: number) => String(value).padStart(2, '0')
  return `${day.getFullYear()}-${pad(day.getMonth() + 1)}-${pad(day.getDate())}`
}

/** The profiles first-run selects. Forks of Firefox are left unchecked. */
export const archivedProfiles: ProfileId[] = [
  'chrome:Default',
  'chrome:Profile 1',
  'firefox:k3x9.default-release',
]

export function archivedVisits(): SyntheticVisit[] {
  return archivedProfiles.flatMap((id) => fixture().visitsByProfile[id])
}

/** Fingerprints of the browsers' own files, to prove PathKeep never wrote to them. */
export function browserFileHashes() {
  return Object.fromEntries(
    Object.entries(fixture().files).map(([id, file]) => [
      id,
      createHash('sha256').update(readFileSync(file)).digest('hex'),
    ]),
  )
}

export async function backend<T>(
  command: string,
  payload: object = {},
): Promise<T> {
  const { devIpcUrl } = resolveDesktopBridgeEnv(process.env)
  const response = await fetch(`${devIpcUrl}/commands/${command}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(payload),
  })
  const body = (await response.json()) as T & {
    message?: string
    code?: string
  }
  // The typed code goes in front, as `code: message` like the backend writes it,
  // so a test can assert either one.
  if (!response.ok) {
    const reason = body.message ?? String(response.status)
    const coded =
      body.code && !reason.startsWith(`${body.code}:`)
        ? `${body.code}: ${reason}`
        : reason
    throw new Error(`${command}: ${coded}`)
  }
  return body
}

/**
 * The archive's visit total once the backend session has the archive open.
 * After the session key is cleared, the window unlocks the archive again from
 * the keychain when it loads; until then the backend refuses. Polls, and on a
 * timeout reports the last refusal.
 */
export async function openArchiveVisitTotal(): Promise<number> {
  let total = 0
  await expect
    .poll(
      async () => {
        try {
          total = (
            await backend<{ totalVisits: number }>('load_dashboard_snapshot')
          ).totalVisits
          return 'open'
        } catch (error) {
          return String(error)
        }
      },
      { timeout: 30_000 },
    )
    .toBe('open')
  return total
}

/**
 * Records an expected/actual pair in the report and asserts it. The report
 * then shows what was checked against what, not only that it passed.
 */
export async function expectCount(
  testInfo: TestInfo,
  name: string,
  actual: number,
  expected: number,
) {
  await testInfo.attach(name, {
    body: JSON.stringify({ expected, actual }),
    contentType: 'application/json',
  })
  expect(actual, name).toBe(expected)
}

/** "34,798" or "34 798" → 34798. */
export function parseCount(text: string): number {
  const digits = text.replace(/[^\d]/g, '')
  if (!digits) throw new Error(`No number in "${text}"`)
  return Number(digits)
}
