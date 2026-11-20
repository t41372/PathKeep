/**
 * Everything the Projects page and a project's page share: every project
 * (for names, the pinned arrangement and the home folder), kept current
 * by the change events, and the dialogs a project's menu opens (Rename,
 * Project settings, Add folder).
 */
import { useMemo, useState, type ReactNode } from 'react'
import { useInsightsFormat, useInsightsZone } from '@/features/insights/zone'
import type {
  ProjectCard,
  ProjectList,
} from '@/lib/backend-client/agent-projects'
import { useAgentProjectsSync } from '@/lib/queries/agent-projects'
import { buildArrangement } from './arrangement'
import { AddFolderDialog, RenameDialog } from './project-dialogs'
import { ProjectSettings } from './project-settings'
import { ProjectsContext, type ProjectsShared } from './projects-context'

export function ProjectsProvider({
  all,
  children,
}: {
  /** Every project (`list_agent_projects` without filters). */
  all: ProjectList | undefined
  children: ReactNode
}) {
  useAgentProjectsSync()
  const zone = useInsightsZone().shown
  const f = useInsightsFormat()
  const [renaming, setRenaming] = useState<ProjectCard | null>(null)
  const [settings, setSettings] = useState<ProjectCard | null>(null)
  const [adding, setAdding] = useState<ProjectCard | null>(null)
  const items = all?.items
  const nodes = all?.arrangement
  const shared = useMemo<ProjectsShared>(() => {
    const cards = new Map((items ?? []).map((card) => [card.id, card]))
    return {
      cards,
      arrangement: buildArrangement(nodes ?? [], cards),
      home: all?.homeFolder ?? null,
      zone,
      f,
      openRename: setRenaming,
      openSettings: setSettings,
    }
  }, [items, nodes, all?.homeFolder, zone, f])
  // An open dialog shows the project as it is now.
  const fresh = (project: ProjectCard | null) =>
    project ? (shared.cards.get(project.id) ?? project) : null
  return (
    <ProjectsContext.Provider value={shared}>
      {children}
      <RenameDialog
        project={fresh(renaming)}
        onClose={() => setRenaming(null)}
      />
      <ProjectSettings
        project={fresh(settings)}
        home={shared.home}
        onAddFolder={setAdding}
        onClose={() => setSettings(null)}
      />
      <AddFolderDialog
        project={fresh(adding)}
        home={shared.home}
        onClose={() => setAdding(null)}
      />
    </ProjectsContext.Provider>
  )
}
