/**
 * The Agent space's projects, for the specs: what the backend says about
 * them (`list_agent_projects`, `get_agent_project`, the suggestions), an
 * edit made through the backend, and putting every project back the way
 * the backups grouped them, so a spec that failed half way leaves no name,
 * pin, section, added folder or answered suggestion behind.
 */
import { expect, type Locator, type Page } from '@playwright/test'
import { unlockIfLocked } from './app'
import { backend } from './fixture'

export interface WireProject {
  id: string
  displayName: string
  defaultName: string
  kind: 'repo' | 'folder' | 'scratch'
  mainFolder: string | null
  parentFolder: string | null
  conversationCount: number
  lastActivity: number | null
  hasAddedFolders: boolean
}

export interface WireProjects {
  version: number
  total: number
  matching: number
  items: WireProject[]
  arrangement: {
    kind: 'section' | 'project'
    id: string
    parent: string | null
    name: string | null
  }[]
  pendingSuggestions: number
  homeFolder: string | null
}

export function wireProjectList(request: Record<string, unknown> = {}) {
  return backend<WireProjects>('list_agent_projects', { request })
}

export function wireProject(id: string) {
  return backend<
    | {
        status: 'ok'
        project: WireProject
        folders: { path: string; role: string }[]
      }
    | { status: 'addedInto'; into: string }
    | { status: 'notFound' }
  >('get_agent_project', { request: { id } })
}

/** One project by its name as the backups gave it. */
export async function projectNamed(name: string) {
  const { items } = await wireProjectList()
  const found = items.find((item) => item.defaultName === name)
  if (!found) throw new Error(`No project is called ${name}`)
  return found
}

async function edit(action: Record<string, unknown>) {
  const { version } = await wireProjectList({ limit: 1 })
  return backend<{ version: number }>('edit_agent_project', {
    request: { version, action },
  })
}

/**
 * Every project back as the backups grouped it: no name of the person's,
 * nothing pinned, no section, no folder added from another project, every
 * suggestion unanswered.
 */
export async function resetProjects() {
  const list = await wireProjectList()
  for (const node of list.arrangement.filter((n) => n.parent === null))
    await edit(
      node.kind === 'section'
        ? { kind: 'deleteSection', id: node.id }
        : { kind: 'unpin', id: node.id },
    )
  for (const item of list.items) {
    if (item.displayName !== item.defaultName && item.kind !== 'scratch')
      await edit({ kind: 'rename', id: item.id, name: '' })
    if (!item.hasAddedFolders) continue
    const detail = await wireProject(item.id)
    if (detail.status !== 'ok') continue
    for (const folder of detail.folders)
      if (folder.role === 'userAdded')
        await edit({ kind: 'detach', id: item.id, path: folder.path })
  }
  const { items } = await backend<{
    items: { id: string; decision: string }[]
  }>('list_agent_project_suggestions', { request: {} })
  for (const suggestion of items)
    if (suggestion.decision !== 'pending')
      await edit({ kind: 'answer', id: suggestion.id, decision: 'pending' })
}

export async function openProjects(page: Page, search = '') {
  await page.goto(`/#/agent/projects${search}`)
  await unlockIfLocked(page)
  await expect(
    page.getByRole('heading', { name: 'Projects', level: 1 }),
  ).toBeVisible()
}

export const pinnedCard = (page: Page) =>
  page.getByRole('region', { name: /^Pinned/ })

export const otherCard = (page: Page) =>
  page.getByRole('region', { name: /^Other projects/ })

/** A project's row on the page or in one card, by its id. */
export const projectRow = (scope: Page | Locator, id: string) =>
  scope.locator(`[data-project-id="${id}"]`)

/** Picks an item of a project row's ⋯ menu. */
export async function projectMenu(
  page: Page,
  id: string,
  name: string,
  item: string | RegExp,
) {
  await projectRow(page, id).hover()
  await projectRow(page, id)
    .getByRole('button', { name: `“${name}” options` })
    .click()
  await page
    .getByRole('menuitem', {
      name: item,
      exact: typeof item === 'string',
    })
    .click()
}

/**
 * Drags by a row's grip and drops at a point of `target` (0 its top edge,
 * 1 its bottom), moving in steps as a hand does, so the pointer sensor
 * starts the drag and the drop zones follow the pointer.
 */
export async function dragTo(
  page: Page,
  grip: Locator,
  target: Locator,
  at = 0.5,
) {
  const from = await grip.boundingBox()
  const to = await target.boundingBox()
  if (!from || !to) throw new Error('Nothing to drag or nowhere to drop')
  await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2)
  await page.mouse.down()
  await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2 + 8, {
    steps: 4,
  })
  const x = to.x + to.width / 2
  const y = to.y + to.height * at
  await page.mouse.move(x, y, { steps: 12 })
  // One more small move where it lands, so the last position is read there.
  await page.mouse.move(x + 1, y)
  await page.mouse.up()
}
