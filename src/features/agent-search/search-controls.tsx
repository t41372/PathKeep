/**
 * The two rows under the Records search box while searching: how the
 * search matches (full text or semantic) and which kinds of text it reads
 * besides titles (`agent-history.md` §3.4). Both modes read the same kinds.
 *
 * Responsible for: drawing and changing those two choices. Not
 * responsible for: the URL (`chats-params.ts`) or running the search.
 */
import { Sparkles } from 'lucide-react'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import type { ChatSearchMode } from '@/features/agent-chats/chats-params'
import {
  SEARCH_ROLES,
  type SearchRole,
} from '@/lib/backend-client/agent-search'
import { cn } from '@/lib/cn'
import { useI18n, type MessageKey } from '@/lib/i18n'

const roleName: Record<SearchRole, MessageKey> = {
  user: 'agentSearch.roles.user',
  agent: 'agentSearch.roles.agent',
  tool: 'agentSearch.roles.tool',
  thinking: 'agentSearch.roles.thinking',
}

const pill =
  'h-6 rounded-md border px-2 text-[12px] transition-colors duration-150'

function Mode({
  mode,
  onMode,
}: {
  mode: ChatSearchMode
  onMode: (mode: ChatSearchMode) => void
}) {
  const { t } = useI18n()
  const item = cn(
    pill,
    'min-w-0 gap-1 font-normal text-muted-foreground shadow-none hover:bg-muted hover:text-foreground data-[state=on]:border-brand/40 data-[state=on]:bg-brand-soft data-[state=on]:text-foreground',
  )
  return (
    <ToggleGroup
      type="single"
      value={mode}
      onValueChange={(next) => next && onMode(next as ChatSearchMode)}
      spacing={1}
      aria-label={t('agentSearch.mode.label')}
    >
      <ToggleGroupItem value="text" className={item}>
        {t('agentSearch.mode.text')}
      </ToggleGroupItem>
      <ToggleGroupItem value="semantic" className={item}>
        <Sparkles className="size-3" aria-hidden />
        {t('agentSearch.mode.semantic')}
      </ToggleGroupItem>
    </ToggleGroup>
  )
}

/**
 * A change of what to search, worked out from the roles as they are when it
 * applies: the buttons may still show the roles of a click ago.
 */
export type RolesChange = (roles: SearchRole[]) => SearchRole[]

/** `roles` with `role` turned off (never the last one) or on, in order. */
function toggleRole(roles: SearchRole[], role: SearchRole) {
  if (roles.includes(role) && roles.length === 1) return roles
  return roles.includes(role)
    ? roles.filter((item) => item !== role)
    : SEARCH_ROLES.filter((item) => item === role || roles.includes(item))
}

function Roles({
  roles,
  onRoles,
}: {
  roles: SearchRole[]
  onRoles: (change: RolesChange) => void
}) {
  const { t } = useI18n()
  return (
    <div
      role="group"
      aria-label={t('agentSearch.roles.label')}
      className="flex flex-wrap items-center gap-1.5"
    >
      <span aria-hidden className="text-[11.5px] text-muted-foreground">
        {t('agentSearch.roles.label')}
      </span>
      {SEARCH_ROLES.map((role) => {
        const on = roles.includes(role)
        // At least one stays on.
        const last = on && roles.length === 1
        return (
          <button
            key={role}
            type="button"
            aria-pressed={on}
            aria-disabled={last || undefined}
            title={last ? t('agentSearch.roles.lastOne') : undefined}
            onClick={() => {
              if (last) return
              onRoles((now) => toggleRole(now, role))
            }}
            className={cn(
              pill,
              on
                ? 'border-brand/40 bg-brand-soft text-foreground'
                : 'border-dashed text-muted-foreground line-through hover:text-foreground',
            )}
          >
            {t(roleName[role])}
          </button>
        )
      })}
    </div>
  )
}

export function SearchControls({
  mode,
  onMode,
  roles,
  onRoles,
}: {
  mode: ChatSearchMode
  onMode: (mode: ChatSearchMode) => void
  roles: SearchRole[]
  onRoles: (change: RolesChange) => void
}) {
  return (
    <div className="flex flex-col gap-1.5 px-4 pt-1 pb-2">
      <Mode mode={mode} onMode={onMode} />
      <Roles roles={roles} onRoles={onRoles} />
    </div>
  )
}
