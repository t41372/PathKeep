/**
 * Fetches Mozilla's Public Suffix List into vault-core's vendor folder, where
 * PathKeep reads it to split hosts into sites the way browsers do. Run it
 * before a release. The list's version line is part of the URL rules'
 * version, so the release builds Insights anew, and an archive split by the
 * older list splits its pages again when it opens (`archive::sites`).
 *
 *   bun run update:public-suffix-list
 */
import { readFileSync, writeFileSync } from 'node:fs'
import path from 'node:path'

// The list asks to be pulled from this address and no other.
const SOURCE = 'https://publicsuffix.org/list/public_suffix_list.dat'
const TARGET = path.resolve(
  'src-tauri/crates/vault-core/vendor/publicsuffix/public_suffix_list.dat',
)
const SECTIONS = [
  '===BEGIN ICANN DOMAINS===',
  '===END ICANN DOMAINS===',
  '===BEGIN PRIVATE DOMAINS===',
  '===END PRIVATE DOMAINS===',
]

const versionOf = (text) => text.match(/^\/\/ VERSION: (.+)$/m)?.[1]?.trim()

const response = await fetch(SOURCE)
if (!response.ok) throw new Error(`${SOURCE} answered ${response.status}`)
const text = await response.text()
for (const section of SECTIONS) {
  if (!text.includes(section))
    throw new Error(`The downloaded list has no ${section} line.`)
}
const next = versionOf(text)
if (!next) throw new Error('The downloaded list names no version.')
const current = versionOf(readFileSync(TARGET, 'utf8'))
if (current === next) {
  console.log(`The public suffix list is already at ${next}.`)
} else {
  writeFileSync(TARGET, text)
  console.log(`Public suffix list ${current} → ${next}.`)
}
