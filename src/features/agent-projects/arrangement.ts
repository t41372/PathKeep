/**
 * The pinned arrangement as a tree (`agent-history.md` §5.2): sections in
 * sections, and pinned projects, in the order the person put them. The
 * backend sends it flat, each node naming its parent, in order.
 *
 * Responsibilities: building the tree, the rows it draws (skipping what is
 * inside a collapsed section), where a node is, the sections something can
 * move into, and the `move` a drop or a menu item means. Not responsible
 * for: sending edits (`use-project-edit.ts`) or drawing.
 */
import type {
  ArrangementNode,
  ProjectAction,
  ProjectCard,
} from '@/lib/backend-client/agent-projects'

export interface TreeNode {
  node: ArrangementNode
  children: TreeNode[]
}

export interface Arrangement {
  roots: TreeNode[]
  byId: Map<string, TreeNode>
  /** Projects pinned anywhere. */
  pinned: Set<string>
}

export function buildArrangement(
  nodes: ArrangementNode[],
  cards: Map<string, ProjectCard>,
): Arrangement {
  const byId = new Map<string, TreeNode>()
  const pinned = new Set<string>()
  for (const node of nodes) {
    // A pin whose project was added into another one is not drawn.
    if (node.kind === 'project' && !cards.has(node.id)) continue
    byId.set(node.id, { node, children: [] })
    if (node.kind === 'project') pinned.add(node.id)
  }
  const roots: TreeNode[] = []
  for (const node of nodes) {
    const entry = byId.get(node.id)
    if (!entry) continue
    const parent = node.parent ? byId.get(node.parent) : undefined
    if (parent) parent.children.push(entry)
    else roots.push(entry)
  }
  return { roots, byId, pinned }
}

/** A row of the pinned card, in drawing order. */
export interface PinnedRow {
  entry: TreeNode
  depth: number
}

export function pinnedRows(arrangement: Arrangement) {
  const rows: PinnedRow[] = []
  const walk = (list: TreeNode[], depth: number) => {
    for (const entry of list) {
      rows.push({ entry, depth })
      if (entry.node.kind === 'section' && !entry.node.collapsed)
        walk(entry.children, depth + 1)
    }
  }
  walk(arrangement.roots, 0)
  return rows
}

/** Every section, in order, with its depth: the "Move to" list. */
export function sectionList(arrangement: Arrangement) {
  const list: { node: ArrangementNode; depth: number }[] = []
  const walk = (entries: TreeNode[], depth: number) => {
    for (const entry of entries)
      if (entry.node.kind === 'section') {
        list.push({ node: entry.node, depth })
        walk(entry.children, depth + 1)
      }
  }
  walk(arrangement.roots, 0)
  return list
}

/** `id` and everything inside it. */
export function selfAndInside(arrangement: Arrangement, id: string) {
  const ids = new Set<string>()
  const walk = (entry: TreeNode | undefined) => {
    if (!entry) return
    ids.add(entry.node.id)
    for (const child of entry.children) walk(child)
  }
  walk(arrangement.byId.get(id))
  return ids
}

/** The siblings `id` is among, and where. */
export function placeOf(arrangement: Arrangement, id: string) {
  const entry = arrangement.byId.get(id)
  if (!entry) return null
  const parent = entry.node.parent
  const siblings = parent
    ? (arrangement.byId.get(parent)?.children ?? [])
    : arrangement.roots
  return {
    parent,
    siblings,
    index: siblings.findIndex((item) => item.node.id === id),
  }
}

/** Projects pinned inside `id`, its sub-sections' included. */
export function projectsInside(arrangement: Arrangement, id: string) {
  let count = 0
  for (const inside of selfAndInside(arrangement, id))
    if (arrangement.byId.get(inside)?.node.kind === 'project') count += 1
  return count
}

/** Where a drop lands relative to the row under the pointer. */
export type DropZone = 'before' | 'after' | 'into'

/**
 * The `move` a drop of `id` at `zone` of `over` means, or `null` when it
 * cannot land there (a section into itself or what is inside it). An
 * open section takes a drop "after" it as its first child.
 */
export function moveFor(
  arrangement: Arrangement,
  id: string,
  over: string,
  zone: DropZone,
): Extract<ProjectAction, { kind: 'move' }> | null {
  if (id === over) return null
  const target = arrangement.byId.get(over)
  if (!target) return null
  if (selfAndInside(arrangement, id).has(over)) return null
  const section = target.node.kind === 'section'
  const without = (list: TreeNode[]) =>
    list.filter((item) => item.node.id !== id)
  if (
    section &&
    (zone === 'into' || (zone === 'after' && !target.node.collapsed))
  )
    return {
      kind: 'move',
      id,
      parent: over,
      index: zone === 'into' ? without(target.children).length : 0,
    }
  const place = placeOf(arrangement, over)
  if (!place) return null
  const siblings = without(place.siblings)
  const at = siblings.findIndex((item) => item.node.id === over)
  return {
    kind: 'move',
    id,
    parent: place.parent,
    index: zone === 'before' ? at : at + 1,
  }
}

/** One step up or down among its siblings; `null` at the end. */
export function nudge(
  arrangement: Arrangement,
  id: string,
  by: -1 | 1,
): Extract<ProjectAction, { kind: 'move' }> | null {
  const place = placeOf(arrangement, id)
  if (!place) return null
  const index = place.index + by
  if (index < 0 || index >= place.siblings.length) return null
  return { kind: 'move', id, parent: place.parent, index }
}

/** The pins and sections, flat, in order: what the backend returns. */
export function orderOf(arrangement: Arrangement) {
  const ids: string[] = []
  const walk = (list: TreeNode[]) => {
    for (const entry of list) {
      ids.push(entry.node.id)
      walk(entry.children)
    }
  }
  walk(arrangement.roots)
  return ids
}
