/**
 * The sources step's numbers for each chosen kind of data: how many sources
 * were found, how many are ticked, how many of those PathKeep can read, and
 * how many need the person (a permission, a folder that can't be read); and
 * the one-line summary the step writes from them.
 *
 * Not responsible for: drawing (`sources-step.tsx` and the panes) or which
 * sources exist (the snapshot, `use-agent-sources.ts`).
 */
import type { AgentToolId, DataKindId } from '@/lib/backend-client/data-kinds'
import type { Translator } from '@/lib/i18n'
import { isBrowserProfileReadable } from '@/lib/platform-guidance'
import type { BrowserProfile } from '@/lib/types'
import { listedProfiles } from './draft'
import type { AgentSources } from './use-agent-sources'

/** Past this many sources a list is grouped and gets a filter. */
export const GROUP_AFTER = 8

/** One row of a pane's list, as grouping and the filter see it. */
export interface ListItem {
  id: string
  /** Rows with the same group sit under one heading in a long list. */
  group: string
  /** What the filter matches. */
  text: string
}

export interface KindCounts {
  kind: DataKindId
  found: number
  chosen: number
  readableChosen: number
  /** Sources PathKeep can't read yet: a permission, or a folder it can't open. */
  attention: number
  /** Still looking; the numbers are what was found so far. */
  counting: boolean
}

export function browsingCounts(
  profiles: BrowserProfile[],
  selected: readonly string[],
): KindCounts {
  const listed = listedProfiles(profiles)
  const chosen = new Set(selected)
  const ticked = listed.filter((profile) => chosen.has(profile.profileId))
  return {
    kind: 'browsing',
    found: listed.length,
    chosen: ticked.length,
    readableChosen: ticked.filter(isBrowserProfileReadable).length,
    attention: listed.filter((profile) => !isBrowserProfileReadable(profile))
      .length,
    counting: false,
  }
}

export function agentCounts(
  sources: AgentSources,
  chosen: readonly AgentToolId[],
): KindCounts {
  const ticked = sources.tools.filter((tool) => chosen.includes(tool.tool))
  return {
    kind: 'agent-history',
    found: sources.tools.length,
    chosen: ticked.length,
    readableChosen: ticked.filter((tool) => tool.readable).length,
    attention: sources.tools.filter((tool) => !tool.readable).length,
    counting: sources.state.phase === 'walking',
  }
}

/** "3 profiles, all backed up", "4 tools, 2 chosen", "No sources found". */
export function summaryText(t: Translator, counts: KindCounts) {
  if (counts.found === 0)
    return t(
      counts.counting
        ? 'onboarding.sources.counting'
        : 'onboarding.sources.nothingFound',
    )
  const unit = counts.kind === 'browsing' ? 'profiles' : 'tools'
  const values = { count: counts.found, chosen: counts.chosen }
  if (counts.chosen === 0) return t(`onboarding.sources.${unit}.none`, values)
  if (counts.chosen === counts.found)
    return t(`onboarding.sources.${unit}.all`, values)
  return t(`onboarding.sources.${unit}.some`, values)
}

/** "1 needs permission" for browsers; "1 can't be read" for Agent tools. */
export function attentionText(t: Translator, counts: KindCounts) {
  if (counts.attention === 0) return null
  return t(
    counts.kind === 'browsing'
      ? 'onboarding.sources.needPermission'
      : 'onboarding.sources.unreadable',
    { count: counts.attention },
  )
}

/** The rows `filter` matches, ignoring case. */
export function filtered<T extends ListItem>(items: T[], filter: string): T[] {
  const needle = filter.trim().toLowerCase()
  if (!needle) return items
  return items.filter((item) => item.text.toLowerCase().includes(needle))
}
