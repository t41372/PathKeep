// Runs insights.spec.ts:628's check (the 台北天氣 family in the Work profile's five most searched)
// on every fixture write-fixtures.mjs wrote. Bun, for the spec's TypeScript helpers.
/* global Bun */
import { readdirSync, readFileSync } from 'node:fs'

const [repo, out] = process.argv.slice(2)
// The fixture module also writes SQLite files, which this check never does.
Bun.plugin({
  setup(build) {
    build.module('node:sqlite', () => ({
      exports: { DatabaseSync: class {} },
      loader: 'object',
    }))
  },
})
const { searchFamilies } = await import(
  `${repo}/tests/e2e/support/search-words.ts`
)
for (const name of readdirSync(out)
  .filter((n) => n.endsWith('.json'))
  .sort()) {
  const fixture = JSON.parse(readFileSync(`${out}/${name}`, 'utf8'))
  const families = searchFamilies(fixture.visitsByProfile['chrome:Profile 1'])
  const taipei = families.filter((f) => f.queries.has('台北天氣'))
  const byKey = new Map()
  for (const f of families) {
    const key = f.anchor.trim().toLowerCase()
    const topic = byKey.get(key)
    if (!topic) byKey.set(key, { key, searches: f.members, latest: f.latest })
    else {
      topic.searches += f.members
      topic.latest = Math.max(topic.latest, f.latest)
    }
  }
  const top = [...byKey.values()]
    .sort(
      (a, b) =>
        b.searches - a.searches ||
        b.latest - a.latest ||
        (a.key < b.key ? -1 : 1),
    )
    .slice(0, 5)
  const pass =
    taipei.length === 1 && top.some((topic) => topic.key === taipei[0].anchor)
  console.log(
    new Date(fixture.now).toUTCString(),
    pass ? 'pass' : 'FAIL',
    taipei.map((f) => `${f.anchor}:${f.members}`).join(),
    '| 5th:',
    `${top[4].key}:${top[4].searches}`,
  )
}
