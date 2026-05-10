#!/usr/bin/env node
/**
 * Prints a sanitized aggregate shape for tuning browser-preview showcase data.
 *
 * ## Responsibilities
 * - Read only aggregate counts from a local PathKeep archive SQLite database.
 * - Avoid selecting URL, title, raw search term, profile path, username, or secret fields.
 * - Produce JSON that can guide synthetic preview fixture proportions.
 *
 * ## Not responsible for
 * - Writing generated source files or mutating the archive.
 * - Exporting real browsing records.
 * - Replacing desktop/runtime tests.
 */

import { existsSync } from 'node:fs'
import { homedir } from 'node:os'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'
import { spawnSync } from 'node:child_process'

const defaultArchive = resolve(
  homedir(),
  'Library/Application Support/com.yi-ting.pathkeep/archive/history-vault.sqlite',
)
const archivePath = resolve(process.argv[2] ?? defaultArchive)

if (!existsSync(archivePath)) {
  console.error(`Archive not found: ${archivePath}`)
  process.exit(1)
}

const databaseUri = `${pathToFileURL(archivePath).href}?mode=ro&immutable=1`

function queryRows(sql) {
  const result = spawnSync('sqlite3', ['-json', databaseUri, sql], {
    encoding: 'utf8',
  })
  if (result.status !== 0) {
    console.error(result.stderr.trim() || 'sqlite3 query failed')
    process.exit(result.status ?? 1)
  }
  return JSON.parse(result.stdout || '[]')
}

const totals = queryRows(`
  select
    count(*) as visits,
    count(distinct url_id) as urls,
    count(distinct source_profile_id) as profiles,
    min(visit_time_iso) as firstVisit,
    max(visit_time_iso) as latestVisit
  from visits
  where reverted_at is null;
`)[0]

const activeHours = queryRows(`
  select
    strftime('%H', visit_time_ms / 1000, 'unixepoch', 'localtime') as hour,
    count(*) as visits
  from visits
  where reverted_at is null
  group by hour
  order by visits desc
  limit 12;
`)

const recentMonths = queryRows(`
  select
    strftime('%Y-%m', visit_time_ms / 1000, 'unixepoch', 'localtime') as month,
    count(*) as visits
  from visits
  where reverted_at is null
  group by month
  order by month desc
  limit 12;
`)

const sourceFamilies = queryRows(`
  select
    coalesce(browser_family, 'unknown') as browserFamily,
    coalesce(browser_product, browser_kind, 'unknown') as browserProduct,
    count(*) as profiles
  from source_profiles
  group by browserFamily, browserProduct
  order by profiles desc;
`)

const searchShape = queryRows(`
  select
    count(*) as searchEvents,
    count(distinct normalized_term) as distinctSearchTerms
  from search_terms
  where reverted_at is null;
`)[0]

const runShape = queryRows(`
  select run_type as runType, status, count(*) as runs
  from runs
  group by run_type, status
  order by runs desc;
`)

console.log(
  JSON.stringify(
    {
      archivePath,
      privacy:
        'Aggregate-only shape. No URLs, titles, usernames, search terms, profile paths, or secrets are selected.',
      totals,
      activeHours,
      recentMonths,
      sourceFamilies,
      searchShape,
      runShape,
    },
    null,
    2,
  ),
)
