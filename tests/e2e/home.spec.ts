/**
 * Home: "Things you keep coming back to" counts days.
 *
 * Proves: a search counts as something the user came back to only when it was
 * searched on more than one day, and the days it shows are the local days
 * with a search in its family, not how many ways it was worded. The fixture's
 * families are simple: one search and its rewordings ("… v2", "… v2 eg"),
 * kept apart per browser profile, so the days are counted here by hand. Every
 * search thread in the archive is checked through the read Home uses, and the
 * search threads on Home itself show the same days.
 */
import { expect, test } from '@playwright/test'
import { archivedProfiles, backend, fixture } from './support/fixture'

const localDay = (at: number) => {
  const day = new Date(at)
  return `${day.getFullYear()}-${day.getMonth()}-${day.getDate()}`
}

/** Days per search family (named by its first wording), per profile. */
function searchDays() {
  return new Map(
    archivedProfiles.map((profile) => {
      const days = new Map<string, Set<string>>()
      for (const visit of fixture().visitsByProfile[profile]) {
        if (!visit.term) continue
        const base = visit.term.replace(/ v2( eg)?$/, '')
        const set = days.get(base) ?? new Set<string>()
        set.add(localDay(visit.at))
        days.set(base, set)
      }
      return [profile as string, days]
    }),
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
