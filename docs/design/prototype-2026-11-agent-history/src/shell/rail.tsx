/**
 * The rail, drawn from the data-type registry.
 *
 * - One type on: its pages go straight into the rail (Agent only →
 *   對話 / 專案 / 洞察), so there is nothing to switch.
 * - Two or more: one item per type; the type's pages become tabs at the top
 *   of the panel. Turning a type on adds one item and moves nothing else.
 * - Four or more: items get shorter; past the window height the middle
 *   scrolls, so the rail fits the 760 px minimum window with any count.
 *
 * Home, Ask, Backup and Settings are shared by every type.
 */
import {
  ChevronLeft,
  ChevronRight,
  Command,
  HardDriveDownload,
  House,
  Lock,
  MessageCircle,
  Moon,
  Plus,
  Settings,
  Sun,
  type LucideIcon,
} from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { NavLink, useLocation, useNavigate } from 'react-router-dom'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { cn } from '@/lib/cn'
import { TaskIndicator } from '~/components/task-indicator'
import { kindById, kinds, pageActive } from '~/kinds'
import { useProto } from '~/proto-state'

function RailLink({
  to,
  icon: Icon,
  label,
  active,
  compact,
}: {
  to: string
  icon: LucideIcon
  label: string
  active?: boolean
  compact?: boolean
}) {
  return (
    <NavLink
      to={to}
      end={to === '/'}
      data-active={active ? '' : undefined}
      className={({ isActive }) =>
        cn(
          'flex w-[60px] shrink-0 flex-col items-center justify-center gap-1 rounded-xl text-muted-foreground transition-[background-color,color,box-shadow] duration-150 hover:text-foreground',
          compact ? 'h-[46px]' : 'h-[58px]',
          (active ?? isActive) && 'bg-card text-foreground shadow-card',
        )
      }
    >
      <Icon className={compact ? 'size-[18px]' : 'size-5'} strokeWidth={1.75} aria-hidden />
      <span className={cn('leading-none font-medium whitespace-nowrap', compact ? 'text-[10px]' : 'text-[11px]')}>{label}</span>
    </NavLink>
  )
}

function RailButton({ label, onClick, compact, children }: { label: string; onClick?: () => void; compact?: boolean; children: React.ReactNode }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          aria-label={label}
          onClick={onClick}
          className={cn('flex w-10 shrink-0 items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-muted hover:text-foreground', compact ? 'h-8' : 'h-9')}
        >
          {children}
        </button>
      </TooltipTrigger>
      <TooltipContent side="right">{label}</TooltipContent>
    </Tooltip>
  )
}

export function Rail({ onOpenPalette }: { onOpenPalette: () => void }) {
  const { enabledKinds, flat, theme, set } = useProto()
  const { pathname } = useLocation()
  const navigate = useNavigate()
  const compact = enabledKinds.length >= 4
  // With many types the "+" moves to Settings → 管理資料類型 to save height.
  const showAdd = enabledKinds.length < kinds.length && !compact
  const middle = useRef<HTMLDivElement>(null)
  const [overflow, setOverflow] = useState({ above: false, below: false })

  // If the types don't fit, say so with fades at the cut edges, and keep the current one in view.
  useEffect(() => {
    const el = middle.current
    if (!el) return
    const measure = () => setOverflow({ above: el.scrollTop > 2, below: el.scrollTop + el.clientHeight < el.scrollHeight - 2 })
    measure()
    el.addEventListener('scroll', measure)
    const ro = new ResizeObserver(measure)
    ro.observe(el)
    return () => {
      el.removeEventListener('scroll', measure)
      ro.disconnect()
    }
  }, [enabledKinds.length, flat])
  useEffect(() => {
    middle.current?.querySelector('[data-active]')?.scrollIntoView({ block: 'nearest' })
  }, [pathname])

  return (
    <nav aria-label="主要導覽" className={cn('flex w-[76px] shrink-0 flex-col items-center gap-1', compact ? 'pb-2.5' : 'pb-4')}>
      {/* Room for the macOS traffic lights, drawn by the frame. */}
      <div className="h-[46px] w-full shrink-0" />
      <div className={cn('flex shrink-0 gap-0.5', compact ? 'mb-1.5' : 'mb-3')}>
        <button type="button" aria-label="上一頁" onClick={() => navigate(-1)} className="flex size-7 items-center justify-center rounded-md text-muted-foreground hover:bg-muted hover:text-foreground">
          <ChevronLeft className="size-[17px]" />
        </button>
        <button type="button" aria-label="下一頁" onClick={() => navigate(1)} className="flex size-7 items-center justify-center rounded-md text-muted-foreground hover:bg-muted hover:text-foreground">
          <ChevronRight className="size-[17px]" />
        </button>
      </div>
      <RailLink to="/" icon={House} label="首頁" compact={compact} />
      <div
        ref={middle}
        className="flex min-h-0 shrink flex-col items-center gap-1 overflow-y-auto [scrollbar-width:none]"
        style={{
          maskImage: `linear-gradient(to bottom, ${overflow.above ? 'transparent' : 'black'}, black 16px, black calc(100% - 16px), ${overflow.below ? 'transparent' : 'black'})`,
        }}
      >
        {flat
          ? kindById[enabledKinds[0]].pages.map((page) => (
              <RailLink key={page.path} to={page.path} icon={page.icon} label={page.label} active={pageActive(page.path, pathname)} />
            ))
          : enabledKinds.map((id) => {
              const kind = kindById[id]
              const prefix = '/' + kind.pages[0].path.split('/')[1] + (kind.real ? '/' : '')
              const active = kind.real ? pathname.startsWith(prefix) : pathname === kind.pages[0].path
              return <RailLink key={id} to={kind.pages[0].path} icon={kind.icon} label={kind.short} active={active} compact={compact} />
            })}
        {showAdd && (
          <Tooltip>
            <TooltipTrigger asChild>
              <NavLink
                to="/settings/kinds"
                aria-label="保存其他類型的資料"
                className="mt-0.5 flex h-8 w-10 shrink-0 items-center justify-center rounded-lg border border-dashed text-muted-foreground transition-colors hover:border-solid hover:bg-muted hover:text-foreground"
              >
                <Plus className="size-4" />
              </NavLink>
            </TooltipTrigger>
            <TooltipContent side="right">保存其他類型的資料</TooltipContent>
          </Tooltip>
        )}
      </div>
      <RailLink to="/ask" icon={MessageCircle} label="助理" compact={compact} />
      <RailLink to="/backup" icon={HardDriveDownload} label="備份" compact={compact} />
      <div className="min-h-1 flex-1" />
      <TaskIndicator />
      <RailButton compact={compact} label="搜尋與指令 ⌘K" onClick={onOpenPalette}>
        <Command className="size-[17px]" />
      </RailButton>
      <RailButton compact={compact} label="鎖定 ⌘L">
        <Lock className="size-[17px]" />
      </RailButton>
      <RailButton compact={compact} label={theme === 'dark' ? '切到淺色' : '切到深色'} onClick={() => set({ theme: theme === 'dark' ? 'light' : 'dark' })}>
        {theme === 'dark' ? <Sun className="size-[18px]" /> : <Moon className="size-[18px]" />}
      </RailButton>
      <RailLink to="/settings" icon={Settings} label="設定" compact={compact} />
    </nav>
  )
}
