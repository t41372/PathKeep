/**
 * Synthetic source mutations for the repair contract. Every path comes from
 * the isolated E2E fixture; these helpers never discover browser directories.
 */
import { DatabaseSync } from 'node:sqlite'
import { mkdirSync } from 'node:fs'
import path from 'node:path'
import { writeSafariHistory } from '../../../scripts/fixtures/synthetic-browsers.mjs'
import { fixture } from './fixture'

export interface NativeVisit {
  id: number
  urlId: number
  url: string
  title: string
  atUs: bigint
  parent?: number
  sync?: boolean
  durationUs?: number | null
}
const chromeEpoch = 11_644_473_600_000_000n
export const unixUs = (ms: number, remainder = 0) =>
  BigInt(ms) * 1000n + BigInt(remainder)

/** Rebuilds the browser tables in place while preserving the file itself. */
export function rebuildChrome(rows: NativeVisit[]) {
  const db = new DatabaseSync(fixture().files['chrome:Default'])
  db.exec(
    'BEGIN; DELETE FROM visits; DELETE FROM urls; DELETE FROM keyword_search_terms; DELETE FROM downloads;',
  )
  const seen = new Set<number>()
  for (const row of rows) {
    if (!seen.has(row.urlId)) {
      db.prepare('INSERT INTO urls VALUES (?,?,?,?,?,?,?)').run(
        row.urlId,
        row.url,
        row.title,
        1,
        0,
        row.atUs + chromeEpoch,
        0,
      )
      seen.add(row.urlId)
    }
    db.prepare('INSERT INTO visits VALUES (?,?,?,?,?,?,?,?,?,?)').run(
      row.id,
      row.urlId,
      row.atUs + chromeEpoch,
      row.parent ?? 0,
      row.parent ? 0 : 1,
      row.durationUs === undefined ? 20_000_000 : row.durationUs,
      row.sync ? 1 : 0,
      0,
      null,
      null,
    )
  }
  db.exec('COMMIT')
  db.close()
}

/** Keeps a real SQLite browser connection open with committed rows in its WAL. */
export function keepChromeOpen() {
  const db = new DatabaseSync(fixture().files['chrome:Default'])
  db.exec(
    'PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; UPDATE visits SET is_known_to_sync=is_known_to_sync;',
  )
  return db
}

export function addTerm(urlId: number, term: string) {
  const db = new DatabaseSync(fixture().files['chrome:Default'])
  db.prepare('INSERT INTO keyword_search_terms VALUES(2,?,?,?)').run(
    urlId,
    term,
    term.toLowerCase(),
  )
  db.close()
}
export function addDownload(guid: string, atUs: bigint) {
  const db = new DatabaseSync(fixture().files['chrome:Default'])
  db.prepare('INSERT INTO downloads VALUES(1,?,?,?,?,?,?,?,?,?)').run(
    guid,
    '/tmp/fixture.part',
    '/tmp/fixture.txt',
    atUs + chromeEpoch,
    10,
    10,
    1,
    'text/plain',
    'text/plain',
  )
  db.close()
}
export function reuseFirefoxId(atUs: bigint) {
  const db = new DatabaseSync(fixture().files['firefox:k3x9.default-release'])
  db.exec('DELETE FROM moz_historyvisits; DELETE FROM moz_places;')
  db.prepare('INSERT INTO moz_places VALUES(1,?,?,1,0,?)').run(
    'https://rescue-firefox.example/new',
    'Firefox after deletion',
    atUs,
  )
  db.prepare('INSERT INTO moz_historyvisits VALUES(1,1,?,0,2)').run(atUs)
  db.close()
}
/** An orphan tests whether coverage compares native rows with joined parser rows. */
export function addOrphan() {
  const db = new DatabaseSync(fixture().files['chrome:Profile 1'])
  db.prepare('INSERT INTO visits VALUES(100,999999,?,0,1,0,0,0,NULL,NULL)').run(
    unixUs(fixture().now) + chromeEpoch,
  )
  db.close()
}
export function removeOrphan() {
  const db = new DatabaseSync(fixture().files['chrome:Profile 1'])
  db.exec('DELETE FROM visits WHERE id=100')
  db.close()
}

/** Reorders old native IDs, then adds a real event only 101 microseconds later. */
export function addLegacyMicrosecondSibling() {
  const db = new DatabaseSync(fixture().files['chrome:Default'])
  db.exec(`UPDATE visits SET id=100-id, from_visit=CASE WHEN from_visit>0 THEN 100-from_visit ELSE from_visit END;
    INSERT INTO visits SELECT 100,url,visit_time+101,0,1,20000000,0,0,NULL,NULL FROM visits WHERE id=79;`)
  db.close()
}

/** Safari can distinguish floating-point timestamps within one microsecond. */
export function preciseSafariFile() {
  const dir = path.join(fixture().root, 'precise-safari')
  mkdirSync(dir, { recursive: true })
  const file = path.join(dir, 'History.db')
  const visit = {
    url: 'https://rescue-safari.example/precise',
    title: 'Safari precise visits',
    at: fixture().now,
    typed: false,
    duration: 20_000,
  }
  writeSafariHistory(file, [visit, visit])
  const db = new DatabaseSync(file)
  const first = fixture().now / 1000 - 978_307_200 + 0.123456
  const update = db.prepare('UPDATE history_visits SET visit_time=? WHERE id=?')
  update.run(first, 1)
  update.run(first + 2 ** -23, 2)
  db.close()
  return file
}
