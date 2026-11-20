/**
 * The controls that narrow a Projects card (`agent-history.md` §5.3–§5.5):
 * the search box (name, any folder's path, remote), the tools menu (any
 * of them), the status toggle and the sort menu.
 */
import { ArrowDownUp, Check, Search, X } from 'lucide-react'
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Input } from '@/components/ui/input'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { ToolIcon } from '@/features/data-kinds/tool-badge'
import { toolName } from '@/features/data-kinds/tools'
import { AGENT_TOOLS, type AgentToolId } from '@/lib/backend-client/data-kinds'
import { useI18n, type MessageKey } from '@/lib/i18n'

export function ProjectSearch({
  value,
  label,
  placeholder,
  onChange,
}: {
  value: string
  label: string
  placeholder: string
  onChange: (value: string) => void
}) {
  const { t } = useI18n()
  return (
    <div className="relative flex min-w-[200px] flex-1 items-center">
      <Search
        className="pointer-events-none absolute left-2.5 size-3.5 text-muted-foreground"
        aria-hidden
      />
      <Input
        type="search"
        value={value}
        aria-label={label}
        placeholder={placeholder}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Escape' && value) {
            event.preventDefault()
            onChange('')
          }
        }}
        className="h-8 pr-8 pl-8 text-[13px] [&::-webkit-search-cancel-button]:hidden"
      />
      {value && (
        <button
          type="button"
          aria-label={t('agentProjects.filters.clearSearch')}
          onClick={() => onChange('')}
          className="absolute right-2 flex size-5 items-center justify-center rounded-full bg-muted text-muted-foreground hover:text-foreground"
        >
          <X className="size-3" aria-hidden />
        </button>
      )}
    </div>
  )
}

export function ToolsMenu({
  value,
  onChange,
}: {
  value: AgentToolId[]
  onChange: (tools: AgentToolId[]) => void
}) {
  const { t } = useI18n()
  const label =
    value.length === 0
      ? t('agentProjects.filters.allTools')
      : value
          .map((tool) => t(toolName(tool)))
          .join(t('agentProjects.filters.toolSeparator'))
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={`${t('agentProjects.filters.toolsLabel')}: ${label}`}
          className="flex h-8 max-w-[220px] items-center gap-1.5 rounded-md border px-2.5 text-[12.5px] text-muted-foreground hover:bg-muted hover:text-foreground"
        >
          <span className="truncate">{label}</span>
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-[220px]">
        <DropdownMenuLabel className="text-[12px] text-muted-foreground">
          {t('agentProjects.filters.toolsLabel')}
        </DropdownMenuLabel>
        <DropdownMenuItem onSelect={() => onChange([])}>
          {t('agentProjects.filters.allTools')}
          {value.length === 0 && <Check className="ml-auto" aria-hidden />}
        </DropdownMenuItem>
        {AGENT_TOOLS.map((tool) => (
          <DropdownMenuCheckboxItem
            key={tool}
            checked={value.includes(tool)}
            onSelect={(event) => event.preventDefault()}
            onCheckedChange={(on) =>
              onChange(
                on
                  ? AGENT_TOOLS.filter(
                      (item) => item === tool || value.includes(item),
                    )
                  : value.filter((item) => item !== tool),
              )
            }
          >
            <ToolIcon id={tool} />
            {t(toolName(tool))}
          </DropdownMenuCheckboxItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

export type ProjectStatus = 'all' | 'recent' | 'gone'

export function StatusToggle({
  value,
  onChange,
}: {
  value: ProjectStatus
  onChange: (status: ProjectStatus) => void
}) {
  const { t } = useI18n()
  const item = 'h-8 px-2.5 text-[12.5px]'
  return (
    <ToggleGroup
      type="single"
      variant="outline"
      value={value}
      aria-label={t('agentProjects.filters.status')}
      onValueChange={(next) => next && onChange(next as ProjectStatus)}
    >
      <ToggleGroupItem value="all" className={item}>
        {t('agentProjects.filters.statusAll')}
      </ToggleGroupItem>
      <ToggleGroupItem value="recent" className={item}>
        {t('agentProjects.filters.statusRecent')}
      </ToggleGroupItem>
      <ToggleGroupItem value="gone" className={item}>
        {t('agentProjects.filters.statusGone')}
      </ToggleGroupItem>
    </ToggleGroup>
  )
}

export type OtherSort = 'activity' | 'conversations' | 'name'
const sortKey: Record<OtherSort, MessageKey> = {
  activity: 'agentProjects.sort.activity',
  conversations: 'agentProjects.sort.conversations',
  name: 'agentProjects.sort.name',
}

export function SortMenu({
  value,
  onChange,
}: {
  value: OtherSort
  onChange: (sort: OtherSort) => void
}) {
  const { t } = useI18n()
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={t('agentProjects.sort.label', {
            sort: t(sortKey[value]),
          })}
          className="flex h-8 items-center gap-1.5 rounded-md border px-2.5 text-[12.5px] text-muted-foreground hover:bg-muted hover:text-foreground"
        >
          <ArrowDownUp className="size-3.5" aria-hidden />
          {t(sortKey[value])}
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        {(Object.keys(sortKey) as OtherSort[]).map((sort) => (
          <DropdownMenuItem key={sort} onSelect={() => onChange(sort)}>
            {t(sortKey[sort])}
            {sort === value && <Check className="ml-auto" aria-hidden />}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
