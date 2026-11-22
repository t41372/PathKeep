/**
 * Round 3 rail: each data type is a space you switch to, like a profile.
 * The switcher sits at the top; under it, always the current space's own
 * pages (so nothing moves between the rail and a top bar). Ask and Settings
 * are shared. The whole archive's status lives in the switcher's menu.
 */
import {
  Check,
  ChevronLeft,
  ChevronRight,
  Command,
  Lock,
  MessageCircle,
  Moon,
  Plus,
  Settings,
  Sun,
  type LucideIcon,
} from 'lucide-react'
import { AnimatePresence, motion, useReducedMotion, type Variants } from 'motion/react'
import { startTransition, useState, type CSSProperties, type ReactNode } from 'react'
import { flushSync } from 'react-dom'
import { NavLink, useLocation, useNavigate } from 'react-router-dom'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { cn } from '@/lib/cn'
import { TaskIndicator } from '~/components/task-indicator'
import { totals } from '~/data/archive'
import { num } from '~/data/format'
import { kindById, kinds, pageActive, type Kind, type KindId } from '~/kinds'
import { useProto } from '~/proto-state'
import { spaceKeyLabel } from './space-key'

/** One line per space for the switcher; the real app reads these from the archive. */
const status: Record<KindId, { line: string; warn?: string }> = {
  browsing: { line: '182 萬筆 · 5 個瀏覽器' },
  agent: { line: `${num(totals.total)} 段對話 · 只剩這裡有 ${num(totals.gone)} 段` },
  terminal: { line: '8.4 萬條指令（示意）' },
  messages: { line: '3.1 萬則訊息（示意）' },
  webchat: { line: '1,240 段對話（示意）' },
  notifications: { line: '2.6 萬則通知（示意）' },
}

/** A small square in the space's own hue, used for the switcher and its menu. */
export function SpaceTile({ kind, size = 'md' }: { kind: Kind; size?: 'sm' | 'md' }) {
  const Icon = kind.icon
  return (
    <span
      style={{ '--h': kind.hue } as CSSProperties}
      className={cn(
        'flex shrink-0 items-center justify-center rounded-[9px] bg-[oklch(0.62_0.12_var(--h)/0.16)] text-[oklch(0.5_0.12_var(--h))] dark:bg-[oklch(0.74_0.11_var(--h)/0.2)] dark:text-[oklch(0.82_0.1_var(--h))]',
        size === 'md' ? 'size-8' : 'size-7',
      )}
    >
      <Icon className={size === 'md' ? 'size-[18px]' : 'size-4'} strokeWidth={1.8} />
    </span>
  )
}

function RailLink({ to, icon: Icon, label, active }: { to: string; icon: LucideIcon; label: string; active?: boolean }) {
  return (
    <NavLink
      to={to}
      className={({ isActive }) =>
        cn(
          'flex h-[54px] w-[60px] shrink-0 flex-col items-center justify-center gap-1 rounded-xl text-muted-foreground transition-[background-color,color,box-shadow] duration-150 hover:text-foreground',
          (active ?? isActive) && 'bg-card text-foreground shadow-card',
        )
      }
    >
      <Icon className="size-5" strokeWidth={1.75} aria-hidden />
      <span className="text-[11px] leading-none font-medium whitespace-nowrap">{label}</span>
    </NavLink>
  )
}

function Tool({ label, onClick, children }: { label: string; onClick?: () => void; children: React.ReactNode }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button type="button" aria-label={label} onClick={onClick} className="flex h-8 w-10 shrink-0 items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-muted hover:text-foreground">
          {children}
        </button>
      </TooltipTrigger>
      <TooltipContent side="right">{label}</TooltipContent>
    </Tooltip>
  )
}

/**
 * Switch spaces. The rail moves first (cheap: the space changes, its items
 * slide); the next space's page renders once the old items have slid out,
 * so a heavy page can't hold the slide back, then fades in.
 */
let pendingPage: ReturnType<typeof setTimeout> | undefined

export function useSwitchSpace() {
  const { set } = useProto()
  const navigate = useNavigate()
  return (id: KindId, to?: string) => {
    // Switching again before the page arrived replaces the pending page, so quick
    // ⌘1 ⌘2 presses end on the last space pressed.
    clearTimeout(pendingPage)
    flushSync(() => set({ space: id }))
    // Let the old items finish sliding out on an idle main thread before the page renders.
    pendingPage = setTimeout(() => startTransition(() => void navigate(to ?? kindById[id].spacePages[0].path)), 170)
  }
}

/**
 * Switching spaces reads as a sideways move: the space's own rail items slide
 * out one way and the next space's slide in from the other, in the order of
 * the switcher (later → content moves left). Shared items below the line
 * stay put. With reduced motion it is only a fade.
 */
function useSlideDirection() {
  const { enabledKinds, space } = useProto()
  const [seen, setSeen] = useState({ space, dir: 1 })
  if (seen.space !== space) {
    const dir = enabledKinds.indexOf(space) >= enabledKinds.indexOf(seen.space) ? 1 : -1
    setSeen({ space, dir })
    return dir
  }
  return seen.dir
}

const DISTANCE = 26
const ease = [0.22, 1, 0.36, 1] as const

// `transform` strings and opacity run as hardware-accelerated animations (WAAPI), so the
// slide keeps moving while the next space's page renders on the main thread.
function itemVariants(still: boolean): Variants {
  const shift = (px: number) => `translateX(${still ? 0 : px}px)`
  return {
    enter: (dir: number) => ({ transform: shift(dir * DISTANCE), opacity: 0 }),
    center: { transform: shift(0), opacity: 1, transition: { duration: 0.26, ease } },
    exit: (dir: number) => ({ transform: shift(-dir * DISTANCE), opacity: 0, transition: { duration: 0.16, ease: 'easeIn' } }),
  }
}

const groupVariants: Variants = {
  enter: {},
  center: { transition: { staggerChildren: 0.035, delayChildren: 0.04 } },
  exit: { transition: { staggerChildren: 0.02 } },
}

/** Re-keys its children per space so they slide; `items` staggers each child. */
function SpaceSlide({ id, dir, children, className }: { id: string; dir: number; children: ReactNode; className?: string }) {
  const still = !!useReducedMotion()
  return (
    <div className={cn('relative', className)}>
      <AnimatePresence initial={false} mode="popLayout" custom={dir}>
        <motion.div key={id} custom={dir} variants={groupVariants} initial="enter" animate="center" exit="exit" className="flex flex-col items-center gap-1">
          {Array.isArray(children)
            ? children.map((child, i) => (
                <motion.div key={i} custom={dir} variants={itemVariants(still)}>
                  {child}
                </motion.div>
              ))
            : (
                <motion.div custom={dir} variants={itemVariants(still)}>
                  {children}
                </motion.div>
              )}
        </motion.div>
      </AnimatePresence>
    </div>
  )
}

function Switcher({ dir }: { dir: number }) {
  const { enabledKinds, space } = useProto()
  const [open, setOpen] = useState(false)
  const switchTo = useSwitchSpace()
  const navigate = useNavigate()
  const kind = kindById[space]
  const more = kinds.length - enabledKinds.length
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label={`目前：${kind.label}。切換資料類型`}
          className="flex w-[64px] shrink-0 flex-col items-center gap-1 rounded-xl py-1.5 transition-colors hover:bg-foreground/5 data-[state=open]:bg-foreground/5"
        >
          <SpaceSlide id={kind.id} dir={dir}>
            <span className="flex flex-col items-center gap-1">
              <SpaceTile kind={kind} />
              <span className="text-[11px] leading-none font-semibold whitespace-nowrap">{kind.short}</span>
            </span>
          </SpaceSlide>
          {/* Page dots: which space of how many, side by side like the slide. */}
          {enabledKinds.length > 1 && (
            <span aria-hidden className="mt-0.5 flex items-center gap-[3px]">
              {enabledKinds.map((id) => (
                <span key={id} className={cn('h-[4px] rounded-full transition-all duration-200', id === space ? 'w-2.5 bg-foreground/70' : 'w-[4px] bg-foreground/20')} />
              ))}
            </span>
          )}
        </button>
      </PopoverTrigger>
      <PopoverContent side="right" align="start" sideOffset={10} className="w-[320px] p-1.5">
        <div className="px-2.5 pt-1.5 pb-2 text-[12px] text-muted-foreground">整個存檔已加密 · 上次備份 今天 15:02</div>
        <div role="listbox" aria-label="資料類型" className="flex flex-col">
          {enabledKinds.map((id, i) => {
            const k = kindById[id]
            const s = status[id]
            return (
              <button
                key={id}
                type="button"
                role="option"
                aria-selected={id === space}
                onClick={() => {
                  setOpen(false)
                  if (id !== space) switchTo(id)
                }}
                className="flex items-center gap-3 rounded-lg px-2.5 py-2 text-left hover:bg-muted"
              >
                <SpaceTile kind={k} size="sm" />
                <span className="flex min-w-0 flex-1 flex-col">
                  <span className="text-[13.5px] font-medium">{k.label}</span>
                  <span className="truncate text-[12px] text-muted-foreground">{s.line}</span>
                </span>
                <span className="flex shrink-0 items-center gap-2 text-[11px] text-muted-foreground">
                  {i < 9 && <kbd className="font-sans">{spaceKeyLabel(i + 1)}</kbd>}
                  <Check className={cn('size-4 text-foreground', id !== space && 'invisible')} />
                </span>
              </button>
            )
          })}
        </div>
        <div className="mt-1 flex flex-col border-t pt-1">
          {more > 0 && (
            <button
              type="button"
              onClick={() => {
                setOpen(false)
                void navigate('/settings/kinds')
              }}
              className="flex items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-[13px] hover:bg-muted"
            >
              <Plus className="size-4 text-muted-foreground" />
              保存其他類型的資料…
              <span className="ml-auto text-[12px] text-muted-foreground">還有 {more} 種</span>
            </button>
          )}
        </div>
      </PopoverContent>
    </Popover>
  )
}

export function SpaceRail({ onOpenPalette }: { onOpenPalette: () => void }) {
  const { space, theme, set } = useProto()
  const { pathname } = useLocation()
  const navigate = useNavigate()
  const kind = kindById[space]
  const dir = useSlideDirection()
  return (
    <nav aria-label="主要導覽" className="flex w-[76px] shrink-0 flex-col items-center gap-1 pb-3">
      <div className="h-[46px] w-full shrink-0" />
      <div className="mb-1.5 flex shrink-0 gap-0.5">
        <button type="button" aria-label="上一頁" onClick={() => navigate(-1)} className="flex size-7 items-center justify-center rounded-md text-muted-foreground hover:bg-muted hover:text-foreground">
          <ChevronLeft className="size-[17px]" />
        </button>
        <button type="button" aria-label="下一頁" onClick={() => navigate(1)} className="flex size-7 items-center justify-center rounded-md text-muted-foreground hover:bg-muted hover:text-foreground">
          <ChevronRight className="size-[17px]" />
        </button>
      </div>
      <Switcher dir={dir} />
      <span className="my-1.5 h-px w-8 shrink-0 bg-foreground/10" />
      <SpaceSlide id={kind.id} dir={dir}>
        {kind.spacePages.map((page) => (
          <RailLink key={page.path} to={page.path} icon={page.icon} label={page.label} active={pageActive(page.path, pathname)} />
        ))}
      </SpaceSlide>
      <div className="min-h-2 flex-1" />
      {/* Below the line: shared by every space. */}
      <span className="mb-1 h-px w-8 shrink-0 bg-foreground/10" />
      <TaskIndicator />
      <RailLink to="/ask" icon={MessageCircle} label="助理" />
      <Tool label="搜尋全部 ⌘K" onClick={onOpenPalette}>
        <Command className="size-[17px]" />
      </Tool>
      <Tool label="鎖定 ⌘L">
        <Lock className="size-[17px]" />
      </Tool>
      <Tool label={theme === 'dark' ? '切到淺色' : '切到深色'} onClick={() => set({ theme: theme === 'dark' ? 'light' : 'dark' })}>
        {theme === 'dark' ? <Sun className="size-[18px]" /> : <Moon className="size-[18px]" />}
      </Tool>
      <RailLink to="/settings" icon={Settings} label="設定" />
    </nav>
  )
}
