// Writes the real E2E fixture for several `now` values; keeps only each one's visits per profile.
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import path from 'node:path'
const [repo, out] = process.argv.slice(2)
const { writeSyntheticBrowsers } = await import(
  `${repo}/scripts/fixtures/synthetic-browsers.mjs`
)
// The failed run's `now`, from its fixture.json.
const failed = 1794707323980
// Every 2 hours across the UTC day of the failed run, and the day before.
const start = new Date(failed)
start.setUTCHours(0, 0, 0, 0)
const nows = [failed]
for (let h = -24; h < 24; h += 2)
  nows.push(start.getTime() + h * 3_600_000 + 30 * 60_000)
for (const now of nows) {
  const dir = mkdtempSync(path.join(out, 'w-'))
  const browsers = writeSyntheticBrowsers(dir, { now, seed: 7 })
  writeFileSync(
    path.join(out, `${now}.json`),
    JSON.stringify({ now, visitsByProfile: browsers.visitsByProfile }),
  )
  rmSync(dir, { recursive: true, force: true })
}
