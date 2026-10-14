/**
 * A URL's site the way browsers and PathKeep split them: by Mozilla's Public
 * Suffix List, both sections, read from the same file the backend vendors
 * (`vault-core/vendor/publicsuffix/`). So `rust-lang.github.io` and
 * `mio-rs.github.io` are two sites, and `news.bbc.co.uk` is on `bbc.co.uk`.
 * The specs count visits per site with it instead of guessing from labels.
 */
import { readFileSync } from 'node:fs'
import path from 'node:path'

interface Rules {
  exact: Set<string>
  wildcard: Set<string>
  exception: Set<string>
}

let cached: Rules | null = null

function rules(): Rules {
  if (cached) return cached
  const file = path.resolve(
    'src-tauri/crates/vault-core/vendor/publicsuffix/public_suffix_list.dat',
  )
  const read: Rules = {
    exact: new Set(),
    wildcard: new Set(),
    exception: new Set(),
  }
  for (const line of readFileSync(file, 'utf8').split('\n')) {
    const rule = line.trim().split(/\s/)[0]
    if (!rule || rule.startsWith('//')) continue
    if (rule.startsWith('!')) read.exception.add(rule.slice(1))
    else if (rule.startsWith('*.')) read.wildcard.add(rule.slice(2))
    else read.exact.add(rule)
  }
  cached = read
  return read
}

/** How many labels at the end of `labels` are the public suffix. */
function suffixLength(labels: string[]) {
  const { exact, wildcard, exception } = rules()
  for (let start = 0; start < labels.length; start += 1)
    if (exception.has(labels.slice(start).join('.')))
      return labels.length - start - 1
  // Without a rule, the last label is the suffix.
  let longest = 1
  for (let start = 0; start < labels.length; start += 1) {
    const candidate = labels.slice(start).join('.')
    const parent = labels.slice(start + 1).join('.')
    if (
      exact.has(candidate) ||
      (start + 1 < labels.length && wildcard.has(parent))
    )
      longest = Math.max(longest, labels.length - start)
  }
  return longest
}

/** The site `url` belongs to; an address, or a host that is itself a suffix, is its own. */
export function siteOf(url: string) {
  const host = new URL(url).hostname.toLowerCase().replace(/\.$/, '')
  if (host.startsWith('[') || /^\d+(\.\d+){3}$/.test(host)) return host
  const labels = host.split('.')
  const suffix = suffixLength(labels)
  return suffix >= labels.length ? host : labels.slice(-(suffix + 1)).join('.')
}
