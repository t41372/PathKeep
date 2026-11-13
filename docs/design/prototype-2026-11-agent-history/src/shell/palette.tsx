/**
 * ⌘K: one box for every enabled data type. Jumps to pages, conversations,
 * projects; "search everything" hands the words to each type's own search,
 * which streams the full result set.
 */
import { FolderGit2, MessagesSquare, Search } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandShortcut,
} from '@/components/ui/command'
import { archive, projectOf, projectsByRecent } from '~/data/archive'
import { when } from '~/data/format'
import { kindById } from '~/kinds'
import { useChatEdits } from '~/pages/agent/chats/chat-edits'
import { useProto } from '~/proto-state'
import { HarnessBadge } from '~/components/bits'
import { spaceKeyLabel } from './space-key'
import { SpaceTile, useSwitchSpace } from './space-rail'

export function Palette({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const navigate = useNavigate()
  const { enabledKinds, nav, space } = useProto()
  const switchTo = useSwitchSpace()
  const [q, setQ] = useState('')
  const agentOn = enabledKinds.includes('agent')
  const go = (to: string) => {
    onOpenChange(false)
    setQ('')
    void navigate(to)
  }
  const needle = q.trim().toLowerCase()
  const { archived, deleted } = useChatEdits()
  const chats = useMemo(() => {
    if (!agentOn) return []
    // Deleted conversations are gone; put-away ones only come up when you type their title.
    const kept = archive.chats.filter((c) => !deleted.has(c.id) && (needle ? c.title.toLowerCase().includes(needle) : !archived.has(c.id)))
    return kept.slice(0, 6)
  }, [needle, agentOn, archived, deleted])
  const projects = useMemo(
    () => (agentOn ? (needle ? projectsByRecent.filter((p) => p.name.toLowerCase().includes(needle)) : projectsByRecent).slice(0, 5) : []),
    [needle, agentOn],
  )

  return (
    <CommandDialog open={open} onOpenChange={onOpenChange} title="搜尋與指令" description="跳到頁面、對話或專案">
      <CommandInput placeholder="搜尋對話、專案、頁面…" value={q} onValueChange={setQ} />
      <CommandList>
        <CommandEmpty>沒有符合的項目</CommandEmpty>
        {needle && (
          <CommandGroup heading="搜尋全部內容">
            {enabledKinds.map((id) => {
              const kind = kindById[id]
              const to = id === 'agent' ? `/agent/chats?q=${encodeURIComponent(q.trim())}` : kind.pages[0].path
              return (
                <CommandItem key={id} value={`search-${id}-${q}`} onSelect={() => go(to)}>
                  <Search />
                  在「{kind.label}」搜尋「{q.trim()}」
                </CommandItem>
              )
            })}
          </CommandGroup>
        )}
        {chats.length > 0 && (
          <CommandGroup heading="Agent 對話">
            {chats.map((chat) => (
              <CommandItem key={chat.id} value={`chat-${chat.id}-${chat.title}`} onSelect={() => go(`/agent/chats?chat=${chat.id}`)}>
                <MessagesSquare />
                <span className="truncate">{chat.title}</span>
                <span className="truncate text-xs text-muted-foreground">{projectOf(chat).name}</span>
                <CommandShortcut className="flex items-center gap-2 tracking-normal">
                  <HarnessBadge id={chat.harness} />
                  {when(chat.ended)}
                </CommandShortcut>
              </CommandItem>
            ))}
          </CommandGroup>
        )}
        {projects.length > 0 && (
          <CommandGroup heading="專案">
            {projects.map((p) => (
              <CommandItem key={p.id} value={`project-${p.id}-${p.name}`} onSelect={() => go(`/agent/projects/${p.id}`)}>
                <FolderGit2 />
                {p.name}
                <span className="truncate text-xs text-muted-foreground">{p.locations[0].path}</span>
              </CommandItem>
            ))}
          </CommandGroup>
        )}
        {nav === 'spaces' && enabledKinds.length > 1 && (
          <CommandGroup heading="切換到">
            {enabledKinds
              .filter((id) => id !== space)
              .map((id, i) => (
                <CommandItem
                  key={id}
                  value={`space-${id}-${kindById[id].label}`}
                  onSelect={() => {
                    onOpenChange(false)
                    setQ('')
                    switchTo(id)
                  }}
                >
                  <SpaceTile kind={kindById[id]} size="sm" />
                  {kindById[id].label}
                  <CommandShortcut>{spaceKeyLabel(enabledKinds.indexOf(id) + 1 || i + 1)}</CommandShortcut>
                </CommandItem>
              ))}
          </CommandGroup>
        )}
        <CommandGroup heading="頁面">
          {enabledKinds.flatMap((id) =>
            (nav === 'spaces' ? kindById[id].spacePages : kindById[id].pages).map((page) => (
              <CommandItem key={page.path} value={`page-${page.path}`} onSelect={() => go(page.path)}>
                <page.icon />
                {kindById[id].label} · {page.label}
              </CommandItem>
            )),
          )}
          {nav === 'types' && (
            <CommandItem value="page-backup" onSelect={() => go('/backup')}>
              備份
            </CommandItem>
          )}
          <CommandItem value="page-tasks" onSelect={() => go('/tasks')}>
            背景任務
          </CommandItem>
          <CommandItem value="page-settings" onSelect={() => go('/settings')}>
            設定
          </CommandItem>
        </CommandGroup>
      </CommandList>
    </CommandDialog>
  )
}
