/**
 * What the report covers (`agent-history.md` §2.1): the range, which is a
 * saved choice and opens the same next time; the tools, which are not
 * kept, since a report silently filtered on the way back would mislead;
 * and the project, which is in the address (`?project=<id>`) so a
 * project's own pages can open its Insights, and nothing else keeps it.
 *
 * Responsibilities: reading and changing those, and the links from the
 * report into History and Projects with the same scope.
 * Not responsible for: the saved choices themselves (`use-preferences.ts`).
 */
import { useCallback, useMemo, useState } from 'react'
import { useSearchParams } from 'react-router-dom'
import type { InsightsRange } from '@/lib/backend-client/agent-insights'
import { AGENT_TOOLS, type AgentToolId } from '@/lib/backend-client/data-kinds'
import { spacePath, type SpaceWithPagesId } from '@/lib/spaces'
import { useInsightsPreferences, useSavePreferences } from './use-preferences'

export interface Scope {
  range: InsightsRange
  harnesses: AgentToolId[]
  projectId: string | null
}

export function useScope() {
  const preferences = useInsightsPreferences()
  const save = useSavePreferences()
  const [params, setParams] = useSearchParams()
  const [harnesses, setHarnesses] = useState<AgentToolId[]>([])
  const projectId = params.get('project') || null
  const range = preferences.data?.range ?? null

  const setRange = useCallback(
    (next: InsightsRange) => void save({ range: next }),
    [save],
  )
  const setProject = useCallback(
    (next: string | null) =>
      setParams(
        (previous) => {
          const changed = new URLSearchParams(previous)
          if (next) changed.set('project', next)
          else changed.delete('project')
          return changed
        },
        { replace: true },
      ),
    [setParams],
  )
  const toggleTool = useCallback(
    (tool: AgentToolId) =>
      setHarnesses((now) =>
        AGENT_TOOLS.filter((id) =>
          id === tool ? !now.includes(id) : now.includes(id),
        ),
      ),
    [],
  )
  const clearFilters = useCallback(() => {
    setHarnesses([])
    setProject(null)
  }, [setProject])

  const scope = useMemo<Scope | null>(
    () => (range ? { range, harnesses, projectId } : null),
    [range, harnesses, projectId],
  )
  return {
    scope,
    preferences,
    setRange,
    toggleTool,
    setProject,
    clearFilters,
  }
}

function filters(scope: Pick<Scope, 'harnesses' | 'projectId'>) {
  const params = new URLSearchParams()
  if (scope.projectId) params.set('project', scope.projectId)
  if (scope.harnesses.length) params.set('h', scope.harnesses.join(','))
  return params
}

/** History with this scope's range, tools and project (§2.2 `對話`). */
export function historyInRange(scope: Scope) {
  const params = filters(scope)
  if (scope.range !== 'all') params.set('range', scope.range)
  const search = params.toString()
  return spacePath('agent', 'chats', search ? `?${search}` : '')
}

/** History on one local day, with this scope's tools and project. */
export function historyOnDay(
  scope: Pick<Scope, 'harnesses' | 'projectId'>,
  day: string,
) {
  const params = filters(scope)
  params.set('day', day)
  return spacePath('agent', 'chats', `?${params.toString()}`)
}

/** One conversation open beside History. */
export function historyWithChat(id: string) {
  return spacePath('agent', 'chats', `?${new URLSearchParams({ chat: id })}`)
}

/**
 * The Projects page and one project's page. The same paths as
 * `agentProjectsPath` / `agentProjectPath` in `src/lib/spaces/paths.ts`
 * once Projects is in the registry.
 */
export function projectsPath() {
  return `/${'agent' satisfies SpaceWithPagesId}/projects`
}

export function projectPath(id: string) {
  return `${projectsPath()}/${encodeURIComponent(id)}`
}
