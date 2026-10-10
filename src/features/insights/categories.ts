/**
 * The kinds of sites the backend sorts visits into (`visit_taxonomy` in
 * vault-core), as Insights names them.
 */
export const UNCLASSIFIED = 'unknown'
const CATEGORIES = [
  'ai',
  'community',
  'developer',
  'docs',
  'education',
  'entertainment',
  'finance',
  'news',
  'search',
  'shopping',
  'social',
  'travel',
  'video',
  'work',
  UNCLASSIFIED,
] as const
type Category = (typeof CATEGORIES)[number]

/** The backend's category, or not classified for one this version has no name for. */
export function category(name: string): Category {
  return CATEGORIES.find((known) => known === name) ?? UNCLASSIFIED
}
