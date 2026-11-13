/**
 * Project rows with the user's folder changes applied, shared by every
 * projects view, plus the two things every row shows the same way: when it
 * was last active and where it lives (so two `api` folders can be told apart).
 */
import { useMemo } from 'react'
import { archive } from '~/data/archive'
import { DAY, NOW } from '~/data/fixture'
import { localDayKey } from '~/data/format'
import type { Project } from '~/data/types'
import { buildRow, type Row } from './derive'
import { displayName, joinedInto, useProjectEdits } from './store'

export function useProjectRows() {
  const edits = useProjectEdits()
  return useMemo(() => {
    const joined = joinedInto(edits)
    const added = new Map<string, Project[]>()
    for (const [from, into] of joined) {
      const list = added.get(into) ?? []
      list.push(archive.projectById.get(from)!)
      added.set(into, list)
    }
    const list = archive.projects.filter((p) => p.chatIds.length > 0 && !joined.has(p.id)).map((p) => buildRow(p, added.get(p.id) ?? []))
    const byId = new Map(list.map((r) => [r.project.id, r]))
    const name = (id: string) => displayName(edits, id)
    // Names that more than one project shows: their rows need a longer locator.
    const seen = new Map<string, number>()
    for (const r of list) seen.set(name(r.project.id), (seen.get(name(r.project.id)) ?? 0) + 1)
    const dupes = new Set([...seen].filter(([, n]) => n > 1).map(([k]) => k))
    const where = (row: Row) => locatorOf(row, dupes.has(name(row.project.id)))
    return { list, byId, name, where, edits }
  }, [edits])
}

export function matches(row: Row, name: string, needle: string) {
  if (!needle) return true
  return (
    name.toLowerCase().includes(needle) ||
    row.project.locations.some((l) => l.path.toLowerCase().includes(needle)) ||
    row.absorbed.some((p) => p.locations.some((l) => l.path.toLowerCase().includes(needle))) ||
    (row.project.remote ?? '').toLowerCase().includes(needle)
  )
}

/** One relative format for every projects view: 今天, 昨天, 5 天前, 3 個月前, 2 年前. */
export function relTime(t: number) {
  if (localDayKey(t) === localDayKey(NOW)) return '今天'
  const days = Math.max(1, Math.round((Date.parse(localDayKey(NOW)) - Date.parse(localDayKey(t))) / DAY))
  if (days === 1) return '昨天'
  if (days < 30) return `${days} 天前`
  if (days < 365) return `${Math.min(11, Math.floor(days / 30))} 個月前`
  return `${Math.floor(days / 365)} 年前`
}

export function dirOf(path: string) {
  const i = path.lastIndexOf('/')
  return i > 0 ? path.slice(0, i) : path
}

export function folderCount(row: Row) {
  return row.project.locations.length + row.absorbed.reduce((s, p) => s + p.locations.length, 0)
}

export interface Locator {
  /** A path, shown in monospace; empty for the scratch bucket. */
  path: string
  /** Plain words after it: 3 個資料夾, 資料夾已不在, 家目錄與暫存資料夾. */
  extra: string
}

/**
 * Where a project lives, muted next to its name: the parent folder, or the
 * full path when another project has the same name; plus how many folders
 * it holds when more than one.
 */
export function locatorOf(row: Row, duplicateName: boolean): Locator {
  const { project } = row
  if (project.kind === 'scratch') return { path: '', extra: '家目錄與暫存資料夾' }
  const main = project.locations[0].path
  const n = folderCount(row)
  return {
    path: duplicateName ? main : dirOf(main),
    extra: [n > 1 ? `${n} 個資料夾` : '', project.gone ? '資料夾已不在' : ''].filter(Boolean).join(' · '),
  }
}

export function locatorText(l: Locator) {
  return [l.path, l.extra].filter(Boolean).join(' · ')
}
