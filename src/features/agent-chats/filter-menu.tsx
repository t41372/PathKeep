/**
 * Above the list: one Filter button (`agent-history.md` §3.2) whose menu
 * holds everything that narrows or orders the list: project, tools, time,
 * "only", sort and group; and under it a removable chip for each condition
 * that is on. The search box goes to the left of the button with search.
 *
 * Responsibilities: the controls and the chips. Not responsible for: the
 * rules between parameters (`chats-params.ts`).
 */
import {
  Check,
  ChevronsUpDown,
  FolderClosed,
  SlidersHorizontal,
  X,
} from 'lucide-react'
import { useState, type ReactNode } from 'react'
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
import { ToolIcon } from '@/features/data-kinds/tool-badge'
import { toolName } from '@/features/data-kinds/tools'
import type {
  ChatFlag,
  ChatGroup,
  ChatRange,
  ChatSort,
} from '@/lib/backend-client/agent-chats'
import { AGENT_TOOLS, type AgentToolId } from '@/lib/backend-client/data-kinds'
import { cn } from '@/lib/cn'
import { useI18n, type Formatters, type MessageKey } from '@/lib/i18n'
import { useProjectList } from '@/lib/queries/agent-projects'
import { ProjectIcon } from '@/features/agent-projects/project-icon'
import { projectName } from '@/features/agent-projects/project-words'
import {
  FLAGS,
  GROUPS,
  RANGES,
  SORTS,
  conditionCount,
  toggled,
  type ChatsParams,
  type ChatsUpdate,
} from './chats-params'

const rangeKey: Record<ChatRange, MessageKey> = {
  all: 'agentChats.filter.range.all',
  today: 'agentChats.filter.range.today',
  '7d': 'agentChats.filter.range.d7',
  '30d': 'agentChats.filter.range.d30',
  '90d': 'agentChats.filter.range.d90',
  '1y': 'agentChats.filter.range.y1',
}
const flagKey = (flag: ChatFlag): MessageKey =>
  `agentChats.filter.flags.${flag}`
const sortKey = (sort: ChatSort): MessageKey =>
  `agentChats.filter.sorts.${sort}`
const groupKey = (group: ChatGroup): MessageKey =>
  `agentChats.filter.groups.${group}`

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
      <div className="flex flex-wrap gap-1.5">{children}</div>
    </div>
  )
}

function Pill({
  on,
  onClick,
  children,
}: {
  on: boolean
  onClick: () => void
  children: ReactNode
}) {
  return (
    <button
      type="button"
      aria-pressed={on}
      onClick={onClick}
      className={cn(
        'flex h-7 items-center gap-1.5 rounded-md border px-2 text-[12.5px] whitespace-nowrap transition-colors duration-150',
        on
          ? 'border-brand/40 bg-brand-soft text-foreground'
          : 'text-muted-foreground hover:bg-muted hover:text-foreground',
      )}
    >
      {children}
    </button>
  )
}

function ProjectField({
  value,
  f,
  onChange,
}: {
  value: string | null
  f: Formatters
  onChange: (project: string | null) => void
}) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  // Every project, put-away conversations' too; the newest activity first.
  const projects = useProjectList({})
  const items = projects.data?.items ?? []
  const picked = value ? items.find((item) => item.id === value) : undefined
  const current = value
    ? picked
      ? projectName(t, picked)
      : (value.split(/[\\/]/).filter(Boolean).pop() ?? value)
    : null
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label={t('agentChats.filter.project')}
          className="flex h-8 w-full items-center gap-1.5 rounded-md border px-2 text-left text-[13px]"
        >
          {current && (
            <FolderClosed className="size-3.5 shrink-0" aria-hidden />
          )}
          <span
            className={cn(
              'flex-1 truncate',
              !current && 'text-muted-foreground',
            )}
          >
            {current ??
              (projects.data
                ? t('agentChats.filter.allProjects', {
                    count: projects.data.total,
                  })
                : t('agentChats.filter.anyProject'))}
          </span>
          <ChevronsUpDown
            className="size-3.5 text-muted-foreground"
            aria-hidden
          />
        </button>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-[320px] p-0">
        <Command>
          <CommandInput placeholder={t('agentChats.filter.findProject')} />
          <CommandList className="max-h-[320px]">
            {projects.data && (
              <CommandEmpty>{t('agentChats.filter.noProject')}</CommandEmpty>
            )}
            <CommandGroup>
              {value && (
                <CommandItem
                  value="__all"
                  onSelect={() => {
                    onChange(null)
                    setOpen(false)
                  }}
                >
                  {t('agentChats.filter.anyProject')}
                </CommandItem>
              )}
              {items.map((project) => (
                <CommandItem
                  key={project.id}
                  value={`${projectName(t, project)} ${project.mainFolder ?? ''} ${project.id}`}
                  onSelect={() => {
                    onChange(project.id)
                    setOpen(false)
                  }}
                >
                  <ProjectIcon project={project} />
                  <span
                    className="truncate"
                    title={project.mainFolder ?? undefined}
                  >
                    {projectName(t, project)}
                  </span>
                  {project.lastActivity !== null && (
                    <span className="ml-auto shrink-0 text-xs text-muted-foreground">
                      {f.relative(project.lastActivity)}
                    </span>
                  )}
                  {project.id === value && <Check className="size-3.5" />}
                </CommandItem>
              ))}
            </CommandGroup>
            {!projects.data && (
              <p
                role="status"
                className="px-3 py-2 text-[12.5px] text-muted-foreground"
              >
                {projects.isError
                  ? t('agentChats.filter.projectsFailed')
                  : t('agentChats.filter.readingProjects')}
              </p>
            )}
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  )
}

export function FilterMenu({
  params,
  update,
  f,
}: {
  params: ChatsParams
  update: ChatsUpdate
  f: Formatters
}) {
  const { t } = useI18n()
  const active = conditionCount(params)
  const toggleTool = (tool: AgentToolId) =>
    update((now) => ({ harnesses: toggled(now.harnesses, tool) }))
  const toggleFlag = (flag: ChatFlag) =>
    update((now) => ({ flags: toggled(now.flags, flag) }))
  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label={
            active
              ? t('agentChats.filter.buttonActive', { count: active })
              : t('agentChats.filter.button')
          }
          className={cn(
            'relative flex h-9 shrink-0 items-center gap-1.5 rounded-lg border px-2.5 text-[13px] text-muted-foreground transition-colors duration-150 hover:bg-muted hover:text-foreground',
            active > 0 && 'border-brand/40 text-foreground',
          )}
        >
          <SlidersHorizontal className="size-4" aria-hidden />
          {active > 0 && (
            <span
              aria-hidden
              className="flex size-4 items-center justify-center rounded-full bg-foreground text-[10px] font-semibold text-background tabular"
            >
              {active}
            </span>
          )}
        </button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-[340px] p-0">
        <div className="flex flex-col divide-y">
          {!params.fixedProject && (
            <div className="flex flex-col gap-1.5 px-3 py-2.5">
              <span className="text-[11.5px] font-medium text-muted-foreground">
                {t('agentChats.filter.project')}
              </span>
              <ProjectField
                value={params.project}
                f={f}
                onChange={(project) => update({ project })}
              />
            </div>
          )}
          <Section title={t('agentChats.filter.tools')}>
            <Pill
              on={params.harnesses.length === 0}
              onClick={() => update({ harnesses: [] })}
            >
              {t('agentChats.filter.allTools')}
            </Pill>
            {AGENT_TOOLS.map((tool) => (
              <Pill
                key={tool}
                on={params.harnesses.includes(tool)}
                onClick={() => toggleTool(tool)}
              >
                <ToolIcon id={tool} />
                {t(toolName(tool))}
              </Pill>
            ))}
          </Section>
          <Section title={t('agentChats.filter.time')}>
            {RANGES.map((range) => (
              <Pill
                key={range}
                on={params.range === range && !params.day}
                onClick={() => update({ range, day: null })}
              >
                {t(rangeKey[range])}
              </Pill>
            ))}
          </Section>
          <Section title={t('agentChats.filter.only')}>
            {FLAGS.map((flag) => (
              <Pill
                key={flag}
                on={params.flags.includes(flag)}
                onClick={() => toggleFlag(flag)}
              >
                {t(flagKey(flag))}
              </Pill>
            ))}
          </Section>
          {/* A search comes in last-activity order, in no groups. */}
          {!params.q && (
            <Section title={t('agentChats.filter.sort')}>
              {SORTS.map((sort) => (
                <Pill
                  key={sort}
                  on={params.sort === sort}
                  onClick={() => update({ sort })}
                >
                  {t(sortKey(sort))}
                </Pill>
              ))}
            </Section>
          )}
          {!params.q && (
            <Section title={t('agentChats.filter.group')}>
              {GROUPS.filter(
                (group) => !(params.fixedProject && group === 'project'),
              ).map((group) => (
                <Pill
                  key={group}
                  on={params.group === group}
                  onClick={() => update({ group })}
                >
                  {t(groupKey(group))}
                </Pill>
              ))}
            </Section>
          )}
        </div>
      </PopoverContent>
    </Popover>
  )
}

/** A removable chip for each condition; "Clear all" when there are two or more. */
export function FilterChips({
  params,
  update,
  projectLabel,
  f,
}: {
  params: ChatsParams
  update: ChatsUpdate
  projectLabel: string | null
  f: Formatters
}) {
  const { t } = useI18n()
  const chips: { key: string; label: ReactNode; clear: () => void }[] = []
  if (params.project && !params.fixedProject)
    chips.push({
      key: 'project',
      label: projectLabel ?? params.project,
      clear: () => update({ project: null }),
    })
  for (const tool of params.harnesses)
    chips.push({
      key: tool,
      label: t(toolName(tool)),
      clear: () =>
        update((now) => ({
          harnesses: now.harnesses.filter((item) => item !== tool),
        })),
    })
  if (params.range !== 'all')
    chips.push({
      key: 'range',
      label: t(rangeKey[params.range]),
      clear: () => update({ range: 'all' }),
    })
  if (params.day)
    chips.push({
      key: 'day',
      label: f.day(params.day, { month: 'long', day: 'numeric' }),
      clear: () => update({ day: null }),
    })
  for (const flag of params.flags)
    chips.push({
      key: flag,
      label: t(flagKey(flag)),
      clear: () =>
        update((now) => ({
          flags: now.flags.filter((item) => item !== flag),
        })),
    })
  if (chips.length === 0) return null
  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {chips.map((chip) => (
        <span
          key={chip.key}
          className="animate-rise flex h-6 max-w-full items-center gap-1 rounded-md bg-brand-soft pr-1 pl-2 text-[12px]"
        >
          <span className="truncate">{chip.label}</span>
          <button
            type="button"
            aria-label={t('agentChats.chips.remove')}
            onClick={chip.clear}
            className="shrink-0 text-muted-foreground hover:text-foreground"
          >
            <X className="size-3" aria-hidden />
          </button>
        </span>
      ))}
      {chips.length > 1 && (
        <button
          type="button"
          className="px-1 text-[12px] text-muted-foreground hover:text-foreground"
          onClick={() =>
            update({
              ...(params.fixedProject ? {} : { project: null }),
              harnesses: [],
              range: 'all',
              day: null,
              flags: [],
            })
          }
        >
          {t('agentChats.chips.clearAll')}
        </button>
      )}
    </div>
  )
}
