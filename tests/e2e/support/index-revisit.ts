/**
 * The visits `semantic-index.spec` adds once the index is built: one to a
 * page as it was, one to a page whose title changed, and later one to a page
 * the archive has never seen. `wipe.spec` counts them in what the archive
 * holds.
 */
import type { FixtureVisit } from './fixture'

/** Words no other page has, so a search for them finds only the retitled page. */
export const RETITLED_TITLE = 'Quokka harbour regatta results'

/** A visit a minute ago to each page, the retitled one under its new title. */
export function indexRevisitVisits(
  same: { url: string; title: string },
  retitled: { url: string },
): FixtureVisit[] {
  const now = Date.now()
  return [
    { ...same, at: now - 120_000, typed: true, duration: 30_000 },
    {
      url: retitled.url,
      title: RETITLED_TITLE,
      at: now - 60_000,
      typed: true,
      duration: 30_000,
    },
  ]
}

/** A page no fixture visit has, with words no other page has. */
export const NEW_PAGE = {
  url: 'https://bakery.example.org/notes/sourdough-starter-feeding-schedule',
  title: 'Sourdough starter feeding schedule',
}

/** A visit a minute ago to `NEW_PAGE`. */
export function newPageVisit(): FixtureVisit {
  return { ...NEW_PAGE, at: Date.now() - 60_000, typed: true, duration: 30_000 }
}

/**
 * A page backed up while a rebuild is paused (`semantic-index.spec`'s
 * cancelled rebuild), with words no other page has.
 */
export const PAUSED_REBUILD_PAGE = {
  url: 'https://tidepool.example.net/field-notes/anemone-survey',
  title: 'Tide pool anemone survey field notes',
}

/** A visit a minute ago to `PAUSED_REBUILD_PAGE`. */
export function pausedRebuildPageVisit(): FixtureVisit {
  return {
    ...PAUSED_REBUILD_PAGE,
    at: Date.now() - 60_000,
    typed: true,
    duration: 30_000,
  }
}

/** Visits `semantic-index.spec` adds to Chrome: `indexRevisitVisits`, `NEW_PAGE` and `PAUSED_REBUILD_PAGE`. */
export function indexRevisitVisitCount() {
  return 4
}
