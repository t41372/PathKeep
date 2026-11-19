/**
 * The left navigation rail, top to bottom: back and forward; the space
 * switcher; the current space's own pages; a line; then what every space
 * shares, which stays put when the space changes: background tasks, Ask,
 * ⌘K, lock, light/dark and Settings (`data-spaces-tradeoff.md` §4.1). The
 * language is chosen in Settings › General, not here.
 *
 * On macOS the traffic lights sit above back and forward; on Windows and
 * Linux the rail starts with them (no brand mark: the three platforms have
 * the same rail, `back-forward-window-frame-tradeoff.md` D).
 *
 * Not responsible for: which pages a space has (the registry,
 * `src/lib/spaces`), switching (`current-space.ts`) or the slide
 * (`space-slide.tsx`).
 */
import {
  ChevronLeft,
  ChevronRight,
  Command,
  Lock,
  MessageCircle,
  Moon,
  Settings,
  Sun,
  type LucideIcon,
} from 'lucide-react'
import { NavLink } from 'react-router-dom'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { backdropDragRegion } from '@/components/app/drag-region'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import {
  hasMacOverlayTitlebar,
  isMacOsHost,
  shortcutLabel,
} from '@/lib/runtime'
import { spacePagePath } from '@/lib/spaces'
import { useTheme } from '@/lib/theme'
import { useCurrentSpace, useSlideDirection } from './current-space'
import { SpaceSlide } from './space-slide'
import { SpaceSwitcher } from './space-switcher'
import { TaskIndicator } from './task-indicator'
import { useHistorySteps } from './use-history-steps'

function RailLink({
  to,
  icon: Icon,
  label,
}: {
  to: string
  icon: LucideIcon
  label: string
}) {
  return (
    <NavLink
      to={to}
      className={({ isActive }) =>
        cn(
          'flex h-[54px] w-[60px] shrink-0 flex-col items-center justify-center gap-1 rounded-xl text-muted-foreground transition-[background-color,color,box-shadow] duration-150 hover:text-foreground',
          isActive && 'bg-card text-foreground shadow-card',
        )
      }
    >
      <Icon className="size-5" strokeWidth={1.75} aria-hidden />
      <span className="text-[11px] leading-none font-medium whitespace-nowrap">
        {label}
      </span>
    </NavLink>
  )
}

function RailButton({
  label,
  onClick,
  children,
}: {
  label: string
  onClick?: () => void
  children: React.ReactNode
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          aria-label={label}
          onClick={onClick}
          className="flex h-8 w-10 shrink-0 items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
        >
          {children}
        </button>
      </TooltipTrigger>
      <TooltipContent side="right">{label}</TooltipContent>
    </Tooltip>
  )
}

function StepButton({
  label,
  disabled,
  onClick,
  children,
}: {
  label: string
  disabled: boolean
  onClick: () => void
  children: React.ReactNode
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          aria-label={label}
          aria-disabled={disabled}
          onClick={disabled ? undefined : onClick}
          className={cn(
            'flex size-7 items-center justify-center rounded-md transition-colors',
            disabled
              ? 'cursor-default text-muted-foreground/40'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground',
          )}
        >
          {children}
        </button>
      </TooltipTrigger>
      <TooltipContent side="right">{label}</TooltipContent>
    </Tooltip>
  )
}

const Divider = ({ className }: { className?: string }) => (
  <span
    aria-hidden
    className={cn('h-px w-8 shrink-0 bg-foreground/10', className)}
  />
)

export function NavRail({
  onOpenPalette,
  onLock,
}: {
  onOpenPalette: () => void
  onLock: () => void
}) {
  const { t } = useI18n()
  const { resolved, setPreference: setTheme } = useTheme()
  const overlayTitlebar = hasMacOverlayTitlebar()
  const steps = useHistorySteps()
  const mac = isMacOsHost()
  const space = useCurrentSpace()
  const dir = useSlideDirection(space)

  return (
    <nav
      aria-label={t('shell.nav.label')}
      className="flex w-[76px] shrink-0 flex-col items-center gap-1 pb-3"
    >
      <div
        {...backdropDragRegion()}
        className={cn('w-full shrink-0', overlayTitlebar ? 'h-[46px]' : 'h-3')}
      />
      <div className="mb-1.5 flex shrink-0 gap-0.5">
        <StepButton
          label={t(mac ? 'shell.nav.backMac' : 'shell.nav.back')}
          disabled={!steps.canBack}
          onClick={steps.back}
        >
          <ChevronLeft className="size-[17px]" />
        </StepButton>
        <StepButton
          label={t(mac ? 'shell.nav.forwardMac' : 'shell.nav.forward')}
          disabled={!steps.canForward}
          onClick={steps.forward}
        >
          <ChevronRight className="size-[17px]" />
        </StepButton>
      </div>
      <SpaceSwitcher dir={dir} />
      <Divider className="my-1.5" />
      <SpaceSlide id={space.id} dir={dir} className="gap-1">
        {space.pages.map((page) => (
          <RailLink
            key={page.id}
            to={spacePagePath(space, page)}
            icon={page.icon}
            label={t(page.label)}
          />
        ))}
      </SpaceSlide>
      {/* Full width: centred in the rail it would be zero wide, and nothing could press it. */}
      <div className="min-h-2 w-full flex-1" {...backdropDragRegion()} />
      {/* Below the line: shared by every space, and still when it changes. */}
      <Divider className="mb-1" />
      <TaskIndicator />
      <RailLink to="/ask" icon={MessageCircle} label={t('shell.nav.ask')} />
      <RailButton
        label={t('shell.nav.palette', { keys: shortcutLabel('K') })}
        onClick={onOpenPalette}
      >
        <Command className="size-[17px]" />
      </RailButton>
      <RailButton
        label={t('shell.nav.lock', { keys: shortcutLabel('L') })}
        onClick={onLock}
      >
        <Lock className="size-[17px]" />
      </RailButton>
      <RailButton
        label={t(
          resolved === 'dark'
            ? 'shell.nav.themeToLight'
            : 'shell.nav.themeToDark',
        )}
        onClick={() => setTheme(resolved === 'dark' ? 'light' : 'dark')}
      >
        {resolved === 'dark' ? (
          <Sun className="size-[18px]" />
        ) : (
          <Moon className="size-[18px]" />
        )}
      </RailButton>
      <RailLink
        to="/settings"
        icon={Settings}
        label={t('shell.nav.settings')}
      />
    </nav>
  )
}
