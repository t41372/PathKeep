/**
 * Home: "Things you keep coming back to" counts days.
 *
 * Proves: a search counts as something the user came back to only when it was
 * searched on more than one day, and the days it shows are the local days
 * with a search in its family, not how many ways it was worded. The families
 * are made by hand with the backend's rule (`support/search-words.ts`): a
 * search and its rewordings ("… v2", "… v2 eg"), and the fixture's two
 * Chinese searches together, kept apart per browser profile. Every search
 * thread in the archive is checked through the read Home uses, and the search
 * threads on Home itself show the same days.
 */
import { expect, test } from '@playwright/test'
import { archivedProfiles, backend, fixture } from './support/fixture'
import { searchFamilies } from './support/search-words'

/** Days per search family (named by its first wording), per profile. */
function searchDays() {
  return new Map(
    archivedProfiles.map((profile) => [
      profile as string,
      new Map(
        searchFamilies(fixture().visitsByProfile[profile]).map((family) => [
          family.anchor,
          family.days,
        ]),
      ),
    ]),
  )
}

interface Thread {
  anchorType: string
  anchorId: string
  anchorLabel: string
  distinctDays: number
}

test('searches come back on several days, counted in days', async ({
  page,
}, testInfo) => {
  const expected = searchDays()
  const today = new Date()
  const end = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, '0')}-${String(today.getDate()).padStart(2, '0')}`
  const { data } = await backend<{ data: { items: Thread[] } }>(
    'get_reopened_investigations',
    { request: { dateRange: { start: '1900-01-01', end }, limit: 1000 } },
  )
  const searches = data.items.filter(
    (item) => item.anchorType === 'query_family',
  )
  const checked = searches.map((item) => {
    // The family id is `family:<profile>:<n>`.
    const profile = item.anchorId.slice(
      'family:'.length,
      item.anchorId.lastIndexOf(':'),
    )
    return {
      label: item.anchorLabel,
      profile,
      days: item.distinctDays,
      expected: expected.get(profile)?.get(item.anchorLabel)?.size,
    }
  })
  await testInfo.attach('search threads, all time', {
    body: JSON.stringify(checked, null, 2),
    contentType: 'application/json',
  })
  expect(checked.length, 'search threads in the archive').toBeGreaterThan(0)
  for (const thread of checked) {
    expect(thread.days, `${thread.label} (${thread.profile})`).toBe(
      thread.expected,
    )
    expect(thread.days).toBeGreaterThan(1)
  }

  await page.goto('/')
  const rows = page
    .getByRole('heading', { name: 'Things you keep coming back to' })
    .locator('xpath=ancestor::section[1]')
    .getByRole('link')
    .filter({ hasText: /\d+ days? · / })
  await expect(rows.first()).toBeVisible({ timeout: 60_000 })
  for (const row of await rows.all()) {
    const text = (await row.innerText()).replace(/\s+/g, ' ')
    const marker = text.search(/\d+ days? · /)
    const label = text.slice(0, marker).trim()
    const days = Number(text.slice(marker).match(/^(\d+)/)?.[1])
    const options = checked.filter((thread) => thread.label === label)
    if (options.length === 0) continue // a page the user reopened
    expect(
      options.map((thread) => thread.days),
      label,
    ).toContain(days)
  }
})

test('a search, its rewordings and another wording of it are one family', async () => {
  // What the fixture means, whatever the grouping rule: each search and its
  // "v2" rewordings are one thing, and so are 台北天氣 and 台北天氣預報 with
  // theirs; a rule that splits them shows the same search twice on Home.
  const familyOf = async (query: string, profileId: string) => {
    const { data } = await backend<{
      data: { families: { familyId: string }[] }
    }>('get_search_families', { request: { query, profileId } })
    return data.families.map((family) => family.familyId)
  }
  for (const profile of archivedProfiles) {
    const terms = new Set(
      fixture()
        .visitsByProfile[profile].filter((visit) => visit.term)
        .map((visit) => visit.term!),
    )
    const groups = new Map<string, string[]>()
    for (const term of terms) {
      const topic = term
        .replace(/ v2( eg)?$/, '')
        .replace(/^台北天氣預報$/, '台北天氣')
      groups.set(topic, [...(groups.get(topic) ?? []), term])
    }
    for (const [topic, wordings] of groups) {
      if (wordings.length < 2) continue
      const families = new Set<string>()
      for (const wording of wordings)
        for (const id of await familyOf(wording, profile)) families.add(id)
      expect(
        families.size,
        `${topic} (${profile}): ${wordings.join(' / ')}`,
      ).toBe(1)
    }
  }
})
