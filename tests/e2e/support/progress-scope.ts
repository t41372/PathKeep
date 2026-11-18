/**
 * Responsibilities: define the progress scenario's browser observations and final visit contribution.
 * Not responsible for writing browser files, product state or calculating counts from the backend.
 */
import { fixture, type FixtureVisit } from './fixture'

/** Shared by the progress scenario and the final wipe so every appended visit is accounted for. */
export function progressScopeVisits(): {
  original: FixtureVisit[]
  updated: FixtureVisit[]
  afterPause: FixtureVisit[]
} {
  const now = fixture().now
  const urls = Array.from(
    { length: 4 },
    (_, i) => `https://progress-scope.example/page/${i}`,
  )
  return {
    original: urls.flatMap((url, group) =>
      Array.from({ length: 40 }, (_, visit) => ({
        url,
        title: `Original progress page ${group}`,
        at: now - 2 * 86_400_000 + group * 60_000 + visit * 1_000,
        typed: true,
        duration: 1_000,
      })),
    ),
    updated: urls.map((url, i) => ({
      url,
      title: `Updated progress page ${i}`,
      at: now + 1_000 + i * 1_000,
      typed: true,
      duration: 1_000,
    })),
    afterPause: [
      {
        url: 'https://progress-scope.example/after-pause',
        title: 'Later unplanned progress document',
        at: now + 10_000,
        typed: true,
        duration: 1_000,
      },
    ],
  }
}

/** The final deletion preview includes all three sets, even though two index scopes handle them. */
export function progressScopeVisitCount() {
  const { original, updated, afterPause } = progressScopeVisits()
  return original.length + updated.length + afterPause.length
}
