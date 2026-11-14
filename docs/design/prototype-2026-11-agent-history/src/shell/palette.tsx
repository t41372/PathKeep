/**
 * ⌘K: one box for every enabled data type. It keeps everything today's
 * palette has (前往, the four 操作, browsing page results with the row that
 * hands the words to History's own search, the key hints) and adds what
 * spaces bring: pages named by space, 切換到, and each other type's results.
 */
import { FolderGit2, Globe, Lock, MessageCircle, MessageCirclePlus, MessagesSquare, Moon, RefreshCw, Search, Settings, Sun, Activity } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
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
  const { enabledKinds, space, theme, set } = useProto()
  const browsingOn = enabledKinds.includes('browsing')
  const switchTo = useSwitchSpace()
  const [q, setQ] = useState('')
  const agentOn = enabledKinds.includes('agent')
  const go = (to: string) => {
    onOpenChange(false)
    setQ('')
    void navigate(to)
  }
  const needle = q.trim().toLowerCase()
  const { archived, trashed } = useChatEdits()
  const chats = useMemo(() => {
    if (!agentOn) return []
    // Trashed conversations never come up; put-away ones only when you type their title.
    const kept = archive.chats.filter((c) => !trashed.has(c.id) && (needle ? c.title.toLowerCase().includes(needle) : !archived.has(c.id)))
    return kept.slice(0, 6)
  }, [needle, agentOn, archived, trashed])
  const projects = useMemo(
    () => (agentOn ? (needle ? projectsByRecent.filter((p) => p.name.toLowerCase().includes(needle)) : projectsByRecent).slice(0, 5) : []),
    [needle, agentOn],
  )

  const matches = (label: string) => !needle || label.toLowerCase().includes(needle)
  const close = () => {
    onOpenChange(false)
    setQ('')
  }
  const pages = [
    ...enabledKinds.flatMap((id) => kindById[id].spacePages.map((page) => ({ to: page.path, icon: page.icon, label: `${kindById[id].label} · ${page.label}` }))),
    { to: '/ask', icon: MessageCircle, label: '助理' },
    { to: '/tasks', icon: Activity, label: '背景任務' },
    { to: '/settings', icon: Settings, label: '設定' },
  ].filter((page) => matches(page.label))
  const actions = [
    { id: 'backup', icon: RefreshCw, label: '立即備份', run: () => toast('原型：這裡會備份所有開著的類型') },
    { id: 'theme', icon: theme === 'dark' ? Sun : Moon, label: '切換深淺色', run: () => set({ theme: theme === 'dark' ? 'light' : 'dark' }) },
    { id: 'lock', icon: Lock, label: '鎖定 PathKeep', hint: '⌘L', run: () => toast('原型：這裡會鎖定 PathKeep；還沒設密碼時會請你先設定') },
    { id: 'chat', icon: MessageCirclePlus, label: '新對話', run: () => void navigate('/ask?new=1') },
  ].filter((action) => matches(action.label))
  // Stand-ins for the pages today's palette finds in the archive once two characters are typed.
  const visited = needle.length >= 2 && browsingOn ? [`${q.trim()} · 維基百科`, `${q.trim()} 是什麼？`, `關於 ${q.trim()} 的討論`] : []

  return (
    <CommandDialog open={open} onOpenChange={(next) => (next ? onOpenChange(true) : close())} title="搜尋與指令" description="跳到頁面、對話或專案" shouldFilter={false}>
      <CommandInput placeholder="搜尋對話、專案、頁面…" value={q} onValueChange={setQ} />
      <CommandList className="max-h-[420px]">
        <CommandEmpty>沒有符合的結果。</CommandEmpty>
        {pages.length > 0 && (
          <CommandGroup heading="前往">
            {pages.map((page) => (
              <CommandItem key={page.to} value={`page-${page.to}`} onSelect={() => go(page.to)}>
                <page.icon />
                {page.label}
              </CommandItem>
            ))}
          </CommandGroup>
        )}
        {actions.length > 0 && (
          <CommandGroup heading="操作">
            {actions.map((action) => (
              <CommandItem
                key={action.id}
                value={`action-${action.id}`}
                onSelect={() => {
                  close()
                  action.run()
                }}
              >
                <action.icon />
                {action.label}
                {action.hint && <CommandShortcut>{action.hint}</CommandShortcut>}
              </CommandItem>
            ))}
          </CommandGroup>
        )}
        {enabledKinds.length > 1 && (
          <CommandGroup heading="切換到">
            {enabledKinds
              .filter((id) => id !== space && matches(kindById[id].label))
              .map((id) => (
                <CommandItem
                  key={id}
                  value={`space-${id}`}
                  onSelect={() => {
                    close()
                    switchTo(id)
                  }}
                >
                  <SpaceTile kind={kindById[id]} size="sm" />
                  {kindById[id].label}
                  <CommandShortcut>{spaceKeyLabel(enabledKinds.indexOf(id) + 1)}</CommandShortcut>
                </CommandItem>
              ))}
          </CommandGroup>
        )}
        {needle.length >= 2 && browsingOn && (
          <CommandGroup heading="頁面">
            {visited.map((title, i) => (
              <CommandItem key={title} value={`visit-${i}`} onSelect={() => go(`/browsing/history?q=${encodeURIComponent(q.trim())}`)}>
                <Globe />
                <span className="truncate">{title}</span>
                <CommandShortcut className="tracking-normal">昨天 14:{10 + i * 7}</CommandShortcut>
              </CommandItem>
            ))}
            <CommandItem value="search-browsing" onSelect={() => go(`/browsing/history?q=${encodeURIComponent(q.trim())}`)}>
              <Search />
              <span className="truncate">在「瀏覽紀錄」搜尋「{q.trim()}」</span>
            </CommandItem>
          </CommandGroup>
        )}
        {agentOn && (chats.length > 0 || needle) && (
          <CommandGroup heading="Agent 對話">
            {chats.map((chat) => (
              <CommandItem key={chat.id} value={`chat-${chat.id}`} onSelect={() => go(`/agent/chats?chat=${chat.id}`)}>
                <MessagesSquare />
                <span className="truncate">{chat.title}</span>
                {archived.has(chat.id) && <span className="shrink-0 text-xs text-muted-foreground">已封存</span>}
                <span className="truncate text-xs text-muted-foreground">{projectOf(chat).name}</span>
                <CommandShortcut className="flex items-center gap-2 tracking-normal">
                  <HarnessBadge id={chat.harness} />
                  {when(chat.ended)}
                </CommandShortcut>
              </CommandItem>
            ))}
            {needle && (
              <CommandItem value="search-agent" onSelect={() => go(`/agent/chats?q=${encodeURIComponent(q.trim())}`)}>
                <Search />
                <span className="truncate">在「Agent 對話」搜尋「{q.trim()}」</span>
              </CommandItem>
            )}
          </CommandGroup>
        )}
        {projects.length > 0 && (
          <CommandGroup heading="專案">
            {projects.map((p) => (
              <CommandItem key={p.id} value={`project-${p.id}`} onSelect={() => go(`/agent/projects/${p.id}`)}>
                <FolderGit2 />
                {p.name}
                <span className="truncate text-xs text-muted-foreground">{p.locations[0].path}</span>
              </CommandItem>
            ))}
          </CommandGroup>
        )}
      </CommandList>
      <div className="flex gap-4 border-t px-3 py-2 text-[11px] text-muted-foreground">
        <span>↑↓ 選擇</span>
        <span>↵ 打開</span>
      </div>
    </CommandDialog>
  )
}
