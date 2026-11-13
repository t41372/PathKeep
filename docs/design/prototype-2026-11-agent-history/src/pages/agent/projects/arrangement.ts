/**
 * 釘選: the user's own arrangement of projects — sections (nest and
 * collapse) and projects placed in them or at the top, in the order the
 * user chose. Everything not placed is "其他專案", sorted by recent activity.
 *
 * This is the user's data. In the product it lives in PathKeep's archive
 * next to renames and folder changes; here localStorage stands in.
 *
 * Rules:
 * - A project PathKeep sees for the first time is not placed: it shows in 其他專案.
 * - Deleting a section never deletes a project: every project inside it (and
 *   in its sub-sections) goes back to 其他專案.
 * - A project whose folders are gone keeps its place (its icon turns grey).
 * - When a project's folders are added to another project, the project they
 *   were added to keeps its place; the other's entry stays in the data, so
 *   moving the folders out again puts it back where it was.
 */
import { useSyncExternalStore } from 'react'
import { archive } from '~/data/archive'

export interface SectionNode {
  kind: 'section'
  id: string
  title: string
  collapsed: boolean
  children: Node[]
}
export interface ProjectNode {
  kind: 'project'
  id: string
}
export type Node = SectionNode | ProjectNode

/** Where something goes: a parent (`null` = top level) and a position in it. */
export interface Place {
  parent: string | null
  index: number
}

interface State {
  root: Node[]
}

const KEY = 'pk-proto-project-sections-v1'

const p = (id: string): ProjectNode => ({ kind: 'project', id })
const sec = (id: string, title: string, children: Node[], collapsed = false): SectionNode => ({ kind: 'section', id, title, collapsed, children })

function seed(): State {
  const acme = archive.projects.filter((x) => x.parent === '~/work/acme' && x.chatIds.length > 0 && x.id !== 'p-lumen').sort((a, b) => b.lastActive - a.lastActive)
  const clients = archive.projects.filter((x) => x.parent === '~/work/clients' && x.chatIds.length > 0).sort((a, b) => b.lastActive - a.lastActive)
  return {
    root: [
      sec('s-common', '常用', [p('p-orbit'), p('p-lumen'), p('p-tide')]),
      // Collapsed, so 其他專案 and its filters are in view without scrolling.
      sec('s-work', '工作', [...acme.slice(0, 4).map((x) => p(x.id)), sec('s-clients', '客戶', clients.slice(0, 3).map((x) => p(x.id)))], true),
      sec('s-thesis', '論文', [p('p-thesis'), p('p-reading')], true),
      sec('s-tools', '小工具', [p('p-dotfiles'), p('p-kiln')], true),
    ],
  }
}

function load(): State {
  try {
    const raw = localStorage.getItem(KEY)
    if (raw) return JSON.parse(raw) as State
  } catch {
    // Private window or broken entry: start from the seed.
  }
  return seed()
}

let state: State = load()
const listeners = new Set<() => void>()

function commit(next: State) {
  state = next
  try {
    localStorage.setItem(KEY, JSON.stringify(state))
  } catch {
    // Not persisted; the arrangement still works for this session.
  }
  for (const l of listeners) l()
}

export function useArrangement(): Node[] {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l)
      return () => listeners.delete(l)
    },
    () => state.root,
  )
}

// ---------------------------------------------------------------- queries

export function findParent(nodes: Node[], id: string, parent: string | null = null): { parent: string | null; index: number } | null {
  for (let i = 0; i < nodes.length; i++) {
    const n = nodes[i]
    if (n.id === id) return { parent, index: i }
    if (n.kind === 'section') {
      const hit = findParent(n.children, id, n.id)
      if (hit) return hit
    }
  }
  return null
}

export function findSection(nodes: Node[], id: string): SectionNode | null {
  for (const n of nodes) {
    if (n.kind !== 'section') continue
    if (n.id === id) return n
    const hit = findSection(n.children, id)
    if (hit) return hit
  }
  return null
}

/** Every project id placed anywhere in the arrangement. */
export function placedIds(nodes: Node[], out = new Set<string>()): Set<string> {
  for (const n of nodes) {
    if (n.kind === 'project') out.add(n.id)
    else placedIds(n.children, out)
  }
  return out
}

export function projectCount(section: SectionNode, visible: (id: string) => boolean): number {
  let n = 0
  for (const c of section.children) n += c.kind === 'project' ? (visible(c.id) ? 1 : 0) : projectCount(c, visible)
  return n
}

/** Sections as an indented list, for the 移到… menu. */
export function sectionTree(nodes: Node[], depth = 0, out: { id: string; title: string; depth: number }[] = []) {
  for (const n of nodes) {
    if (n.kind !== 'section') continue
    out.push({ id: n.id, title: n.title, depth })
    sectionTree(n.children, depth + 1, out)
  }
  return out
}

/** True when `id` is the section `ancestorId` or somewhere inside it. */
export function contains(nodes: Node[], ancestorId: string, id: string | null): boolean {
  if (id === null) return false
  if (id === ancestorId) return true
  const section = findSection(nodes, ancestorId)
  return !!section && !!findSection(section.children, id)
}

// -------------------------------------------------------------- mutations

const clone = (nodes: Node[]): Node[] => nodes.map((n) => (n.kind === 'section' ? { ...n, children: clone(n.children) } : { ...n }))

function childrenOf(root: Node[], parent: string | null): Node[] {
  if (parent === null) return root
  return findSection(root, parent)!.children
}

function detach(root: Node[], id: string): Node | null {
  const at = findParent(root, id)
  if (!at) return null
  return childrenOf(root, at.parent).splice(at.index, 1)[0]
}

/**
 * Put a project (placed or not) or a section at `to`; `to = null` sends a
 * project back to 其他專案. Returns false when the move makes no sense
 * (a section into itself).
 */
export function move(id: string, kind: Node['kind'], to: Place | null): boolean {
  const root = clone(state.root)
  if (kind === 'section' && to && contains(root, id, to.parent)) return false
  const from = findParent(root, id)
  const node = detach(root, id) ?? (kind === 'project' ? p(id) : null)
  if (!node) return false
  if (to) {
    let index = to.index
    // Moving down inside the same list: the removal shifted everything up by one.
    if (from && from.parent === to.parent && from.index < index) index--
    const list = childrenOf(root, to.parent)
    list.splice(Math.max(0, Math.min(index, list.length)), 0, node)
  }
  commit({ root })
  return true
}

/** Up or down among its siblings. */
export function nudge(id: string, delta: -1 | 1) {
  const at = findParent(state.root, id)
  if (!at) return
  const siblings = childrenOf(state.root, at.parent)
  const target = at.index + delta
  if (target < 0 || target >= siblings.length) return
  const kind = siblings[at.index].kind
  move(id, kind, { parent: at.parent, index: delta > 0 ? target + 1 : target })
}

let counter = 0
export function createSection(parent: string | null, title = '新區塊'): string {
  const root = clone(state.root)
  const id = `s-${Date.now().toString(36)}-${counter++}`
  const node = sec(id, title, [])
  if (parent === null) root.unshift(node)
  else {
    const s = findSection(root, parent)!
    s.collapsed = false
    s.children.push(node)
  }
  commit({ root })
  return id
}

export function renameSection(id: string, title: string) {
  const root = clone(state.root)
  const s = findSection(root, id)
  if (!s || !title.trim()) return
  s.title = title.trim()
  commit({ root })
}

export function setCollapsed(id: string, collapsed: boolean) {
  const root = clone(state.root)
  const s = findSection(root, id)
  if (!s) return
  s.collapsed = collapsed
  commit({ root })
}

export function setAllCollapsed(collapsed: boolean) {
  const walk = (nodes: Node[]): Node[] => nodes.map((n) => (n.kind === 'section' ? { ...n, collapsed, children: walk(n.children) } : n))
  commit({ root: walk(state.root) })
}

/** Removes the section and its sub-sections; their projects go back to 其他專案. */
export function deleteSection(id: string) {
  const root = clone(state.root)
  detach(root, id)
  commit({ root })
}

export function resetArrangement() {
  commit(seed())
}

export function snapshot(): Node[] {
  return state.root
}

export function restore(root: Node[]) {
  commit({ root })
}
