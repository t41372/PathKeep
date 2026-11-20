/**
 * How a project is named and placed in words (`agent-history.md` §5.1):
 * its name (the loose folders have one of their own), the locator that
 * tells two projects of the same name apart, paths with the home folder
 * as `~`, and how long ago it was last active.
 */
import type { ProjectCard } from '@/lib/backend-client/agent-projects'
import type { Translator } from '@/lib/i18n'
import { daysBetween, dateKeyIn, todayIn } from '@/lib/time-zone'

export function projectName(
  t: Translator,
  project: Pick<ProjectCard, 'displayName' | 'kind'>,
) {
  return (
    project.displayName ||
    (project.kind === 'scratch' ? t('agentProjects.scratchName') : '')
  )
}

/** `path` with the home folder written as `~`. */
export function tildePath(path: string, home: string | null) {
  if (!home) return path
  const base = home.replace(/[\\/]+$/, '')
  if (path === base) return '~'
  for (const separator of ['/', '\\'])
    if (path.startsWith(base + separator))
      return `~${separator}${path.slice(base.length + 1)}`
  return path
}

/**
 * The words beside a project's name: where its main folder is (its parent;
 * the whole path when another project has the same name), how many
 * folders, whether they are gone. The loose folders have no place.
 */
export function projectLocator(
  t: Translator,
  project: ProjectCard,
  home: string | null,
) {
  if (project.kind === 'scratch') return ''
  const place = project.nameIsShared ? project.mainFolder : project.parentFolder
  const parts = [place ? tildePath(place, home) : null]
  if (project.folderCount > 1)
    parts.push(t('agentProjects.row.folders', { count: project.folderCount }))
  if (project.gone) parts.push(t('agentProjects.row.gone'))
  return parts.filter(Boolean).join(' · ')
}

/** Today, yesterday, n days, months or years ago, by calendar day in `zone`. */
export function lastActive(
  t: Translator,
  at: number | null,
  zone: string,
  now = Date.now(),
) {
  if (at === null) return t('agentProjects.row.noActivity')
  const days = Math.max(0, daysBetween(dateKeyIn(at, zone), todayIn(zone, now)))
  if (days === 0) return t('agentProjects.age.today')
  if (days === 1) return t('agentProjects.age.yesterday')
  if (days < 30) return t('agentProjects.age.days', { count: days })
  if (days < 365)
    return t('agentProjects.age.months', { count: Math.floor(days / 30) })
  return t('agentProjects.age.years', { count: Math.floor(days / 365) })
}

/** The grid every view's header and rows share. */
export function gridOf(list: boolean) {
  return list
    ? 'grid grid-cols-[minmax(0,1fr)_88px_72px_96px_88px_104px_36px] items-center gap-3'
    : 'grid grid-cols-[minmax(0,1fr)_88px_72px_104px_36px] items-center gap-3'
}
