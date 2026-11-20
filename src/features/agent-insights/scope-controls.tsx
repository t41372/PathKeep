/**
 * The header's controls (`agent-history.md` §2.1): the range, always in
 * sight, and one Filter button for the tools and the project, which says
 * what is on. The project list reads every project at once and finds in
 * it as you type; picking the chosen one again, or "All projects", clears
 * it.
 *
 * Responsibilities: the controls and the project list they read.
 * Not responsible for: where the choices are kept (`scope.ts`).
 */
import { Check, SlidersHorizontal, X } from 'lucide-react'
import { useState, type ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from '@/components/ui/command'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { ProjectIcon } from '@/features/agent-projects/project-icon'
import {
  projectLocator,
  projectName,
} from '@/features/agent-projects/project-words'
import { ToolIcon } from '@/features/data-kinds/tool-badge'
import { toolName } from '@/features/data-kinds/tools'
import type { InsightsRange } from '@/lib/backend-client/agent-insights'
import type { ProjectCard } from '@/lib/backend-client/agent-projects'
import { AGENT_TOOLS, type AgentToolId } from '@/lib/backend-client/data-kinds'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { useProjectList } from '@/lib/queries/agent-projects'
import type { Scope } from './scope'
import { RANGES, rangeKey } from './words'

export function RangeToggle({
  range,
  onRange,
}: {
  range: InsightsRange
  onRange: (range: InsightsRange) => void
}) {
  const { t } = useI18n()
  return (
    <ToggleGroup
      type="single"
      value={range}
      aria-label={t('agentInsights.range.label')}
      onValueChange={(next) => {
        if (next) onRange(next as InsightsRange)
      }}
      className="rounded-[9px] bg-muted p-[3px]"
    >
      {RANGES.map((id) => (
        <ToggleGroupItem
          key={id}
          value={id}
          className="h-[30px] rounded-[7px] px-3 text-[13px] whitespace-nowrap data-[state=on]:bg-card data-[state=on]:shadow-card"
        >
          {t(rangeKey(id))}
        </ToggleGroupItem>
      ))}
    </ToggleGroup>
  )
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div
      role="group"
      aria-label={title}
      className="flex flex-col gap-1.5 px-3 py-2.5"
    >
      <span
        aria-hidden
        className="text-[11.5px] font-medium text-muted-foreground"
      >
        {title}
      </span>
      {children}
    </div>
  )
}

export function FilterButton({
  scope,
  project,
  onTool,
  onProject,
}: {
  scope: Scope
  /** The chosen project, once the list has it. */
  project: ProjectCard | undefined
  onTool: (tool: AgentToolId) => void
  onProject: (id: string | null) => void
}) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const projects = useProjectList({}, open || scope.projectId !== null)
  const conditions = scope.harnesses.length + (scope.projectId ? 1 : 0)
  const list = projects.data
  const tools =
    scope.harnesses.length === 1
      ? t(toolName(scope.harnesses[0]))
      : t('agentInsights.filter.toolCount', { count: scope.harnesses.length })

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          size="sm"
          aria-label={
            conditions
              ? t('agentInsights.filter.some', { count: conditions })
              : t('agentInsights.filter.none')
          }
          className={cn(
            'h-9 max-w-[320px] bg-card font-normal',
            conditions > 0 && 'border-brand/50',
          )}
        >
          <SlidersHorizontal className="text-muted-foreground" aria-hidden />
          {conditions === 0 && t('agentInsights.filter.button')}
          {scope.projectId && (
            <span className="max-w-[140px] truncate">
              {project
                ? projectName(t, project)
                : t('agentInsights.filter.project')}
            </span>
          )}
          {scope.harnesses.length > 0 && <span>{tools}</span>}
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-[340px] p-0">
        <Section title={t('agentInsights.filter.tools')}>
          <div className="flex flex-wrap gap-1.5">
            {AGENT_TOOLS.map((tool) => {
              const on = scope.harnesses.includes(tool)
              return (
                <button
                  key={tool}
                  type="button"
                  aria-pressed={on}
                  onClick={() => onTool(tool)}
                  className={cn(
                    'flex h-7 items-center gap-1.5 rounded-md border px-2 text-[12.5px] whitespace-nowrap transition-colors duration-150',
                    on
                      ? 'border-brand/40 bg-brand-soft text-foreground'
                      : 'text-muted-foreground hover:bg-muted hover:text-foreground',
                  )}
                >
                  <ToolIcon id={tool} />
                  {t(toolName(tool))}
                </button>
              )
            })}
          </div>
        </Section>
        <div className="border-t" />
        <div className="flex items-center justify-between px-3 pt-2.5 text-[11.5px] font-medium text-muted-foreground">
          {t('agentInsights.filter.project')}
          {scope.projectId && (
            <button
              type="button"
              className="font-normal hover:text-foreground"
              onClick={() => onProject(null)}
            >
              {t('agentInsights.filter.allProjects')}
            </button>
          )}
        </div>
        <Command>
          <CommandInput
            placeholder={t('agentInsights.filter.find', {
              count: list?.total ?? 0,
            })}
          />
          <CommandList className="max-h-[260px]">
            {projects.isError ? (
              <p className="px-3 py-4 text-center text-[13px] text-muted-foreground">
                {t('agentInsights.filter.failed')}
              </p>
            ) : !list ? (
              <p className="px-3 py-4 text-center text-[13px] text-muted-foreground">
                {t('agentInsights.filter.loading')}
              </p>
            ) : (
              <>
                <CommandEmpty>{t('agentInsights.filter.noMatch')}</CommandEmpty>
                <CommandGroup>
                  {list.items.map((option) => {
                    const name = projectName(t, option)
                    const place = projectLocator(t, option, list.homeFolder)
                    return (
                      <CommandItem
                        key={option.id}
                        value={`${name} ${option.mainFolder ?? ''} ${option.id}`}
                        onSelect={() =>
                          onProject(
                            option.id === scope.projectId ? null : option.id,
                          )
                        }
                      >
                        <ProjectIcon project={option} className="size-3.5" />
                        <span className="truncate">{name}</span>
                        <span className="ml-auto truncate text-xs text-muted-foreground">
                          {place}
                        </span>
                        {scope.projectId === option.id && (
                          <Check className="size-3.5" aria-hidden />
                        )}
                      </CommandItem>
                    )
                  })}
                </CommandGroup>
              </>
            )}
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  )
}

/** Under the title while a project is chosen: which one, and a way back to all. */
export function ProjectLine({
  project,
  onClear,
}: {
  project: ProjectCard | undefined
  onClear: () => void
}) {
  const { t } = useI18n()
  return (
    <span className="flex items-center gap-1.5 text-[13px] text-muted-foreground">
      {t('agentInsights.project.only')}
      {project && (
        <span className="flex min-w-0 items-center gap-1 font-medium text-foreground">
          <ProjectIcon project={project} className="size-3.5" />
          <span className="truncate">{projectName(t, project)}</span>
        </span>
      )}
      <button
        type="button"
        aria-label={t('agentInsights.project.clear')}
        title={t('agentInsights.project.clear')}
        onClick={onClear}
        className="inline-flex size-5 items-center justify-center rounded-md transition-colors hover:bg-muted hover:text-foreground"
      >
        <X className="size-3.5" aria-hidden />
      </button>
    </span>
  )
}
