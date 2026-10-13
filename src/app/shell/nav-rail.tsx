/**
 * The left navigation rail: back and forward at the top, the five sections,
 * then tools. On macOS the traffic lights sit above back and forward; on
 * Windows and Linux the rail starts with them (no brand mark: the three
 * platforms have the same rail, `back-forward-window-frame-tradeoff.md` D).
 */
import {
  ChartColumn,
  ChevronLeft,
  ChevronRight,
  Command,
  HardDriveDownload,
  History,
  House,
  Lock,
  MessageCircle,
  Moon,
  Settings,
  Sun,
  type LucideIcon,
} from 'lucide-react'
import { NavLink } from 'react-router-dom'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { cn } from '@/lib/cn'
import {
  languageNames,
  supportedLanguages,
  useI18n,
  type MessageKey,
} from '@/lib/i18n'
import { useChooseLanguage } from '@/lib/queries/language'
import { hasMacOverlayTitlebar, isMacOsHost } from '@/lib/runtime'
import { useTheme } from '@/lib/theme'
import type { LanguagePreference } from '@/lib/types'
import { useHistorySteps } from './use-history-steps'

const sections: { to: string; icon: LucideIcon; label: MessageKey }[] = [
  { to: '/', icon: House, label: 'shell.nav.home' },
  { to: '/history', icon: History, label: 'shell.nav.history' },
  { to: '/insights', icon: ChartColumn, label: 'shell.nav.insights' },
  { to: '/ask', icon: MessageCircle, label: 'shell.nav.ask' },
  { to: '/backup', icon: HardDriveDownload, label: 'shell.nav.backup' },
]

const languageShort: Record<LanguagePreference, string> = {
  system: 'A',
  en: 'EN',
  'zh-CN': '简',
  'zh-TW': '繁',
}

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
      end={to === '/'}
      className={({ isActive }) =>
        cn(
          'flex h-[58px] w-[60px] flex-col items-center justify-center gap-1 rounded-xl text-muted-foreground transition-[background-color,color,box-shadow] duration-150 hover:text-foreground',
          isActive && 'bg-card text-foreground shadow-card',
        )
      }
    >
      <Icon className="size-5" strokeWidth={1.75} aria-hidden />
      <span className="text-[11px] font-medium">{label}</span>
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
          className="flex h-9 w-10 items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
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

export function NavRail({
  onOpenPalette,
  onLock,
}: {
  onOpenPalette: () => void
  onLock: () => void
}) {
  const { t, preference, language } = useI18n()
  const chooseLanguage = useChooseLanguage()
  const { resolved, setPreference: setTheme } = useTheme()
  const overlayTitlebar = hasMacOverlayTitlebar()
  const steps = useHistorySteps()
  const mac = isMacOsHost()

  return (
    <nav
      aria-label={t('shell.nav.label')}
      className="flex w-[76px] shrink-0 flex-col items-center gap-1 pb-4"
    >
      <div
        data-tauri-drag-region
        className={cn('w-full shrink-0', overlayTitlebar ? 'h-[46px]' : 'h-3')}
      />
      <div className="mb-3 flex shrink-0 gap-0.5">
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
      {sections.map((section) => (
        <RailLink key={section.to} {...section} label={t(section.label)} />
      ))}
      <div className="flex-1" data-tauri-drag-region />
      <RailButton label={t('shell.nav.palette')} onClick={onOpenPalette}>
        <Command className="size-[17px]" />
      </RailButton>
      <RailButton label={t('shell.nav.lock')} onClick={onLock}>
        <Lock className="size-[17px]" />
      </RailButton>
      <DropdownMenu>
        <Tooltip>
          <TooltipTrigger asChild>
            <DropdownMenuTrigger
              aria-label={t('shell.nav.language')}
              className="flex h-8 w-10 items-center justify-center rounded-lg text-xs font-medium text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
            >
              {languageShort[preference === 'system' ? language : preference]}
            </DropdownMenuTrigger>
          </TooltipTrigger>
          <TooltipContent side="right">
            {t('shell.nav.language')}
          </TooltipContent>
        </Tooltip>
        <DropdownMenuContent side="right" align="end">
          <DropdownMenuRadioGroup
            value={preference}
            onValueChange={(value) =>
              chooseLanguage(value as LanguagePreference)
            }
          >
            <DropdownMenuRadioItem value="system">
              {t('shell.theme.system')}
            </DropdownMenuRadioItem>
            {supportedLanguages.map((lang) => (
              <DropdownMenuRadioItem key={lang} value={lang}>
                {languageNames[lang]}
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>
        </DropdownMenuContent>
      </DropdownMenu>
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
