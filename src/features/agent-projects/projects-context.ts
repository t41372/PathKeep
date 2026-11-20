/**
 * What every part of the Projects screens shares: the cards by id, the
 * pinned arrangement, the home folder (for `~`), the insights zone, and
 * the dialogs any row can open. The page provides it; rows and menus read
 * it instead of passing it down through every view.
 */
import { createContext, useContext, type RefObject } from 'react'
import type { ProjectCard } from '@/lib/backend-client/agent-projects'
import type { Formatters } from '@/lib/i18n'
import type { Arrangement } from './arrangement'

export interface ProjectsShared {
  cards: Map<string, ProjectCard>
  arrangement: Arrangement
  home: string | null
  zone: string
  f: Formatters
  openRename: (project: ProjectCard) => void
  openSettings: (project: ProjectCard) => void
}

export const ProjectsContext = createContext<ProjectsShared | null>(null)

export function useProjectsShared() {
  const value = useContext(ProjectsContext)
  if (!value) throw new Error('useProjectsShared needs the Projects page')
  return value
}

/** A project row's height: rows are virtualized at this size. */
export const ROW_HEIGHT = 40

/** The page's scrolling element, which every card's rows follow. */
export const PageScrollContext = createContext<RefObject<HTMLElement | null>>({
  current: null,
})

/**
 * For a sheet's `onInteractOutside`: a click on a notice (its Undo) is not
 * a click away from the sheet, so the sheet stays open.
 */
export function keepOpenForNotices(event: {
  target: EventTarget | null
  preventDefault: () => void
}) {
  if (
    event.target instanceof Element &&
    event.target.closest('[data-sonner-toaster]')
  )
    event.preventDefault()
}
