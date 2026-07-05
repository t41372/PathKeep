/**
 * Paper-redesign topbar: back/forward nav + serif page title + ⌘K search
 * trigger + Backup CTA.
 *
 * Why this file exists:
 * - The topbar is read first on every route render; getting its serif sizing
 *   right is what makes the app feel like a book rather than a SaaS dashboard.
 * - It also hosts the global route back/forward control. The v0.2 chrome
 *   shipped that affordance and users got used to it (mirrors browser
 *   behaviour, lets you undo a sidebar mis-click); the v0.3 paper shell
 *   was missing it before this restoration.
 *
 * Not responsible for:
 * - Search palette rendering (delegates to PKSearchPalette via onOpenPalette).
 * - Running backup (delegates to onBackupNow).
 * - Owning the actual navigation history stack — react-router-dom does
 *   that, and `useRouteHistoryNav` (see ./use-route-history-nav.ts)
 *   wraps the policy of "is forward currently possible".
 */

import { useI18n } from '@/lib/i18n/hooks'
import type { AppScreen } from '@/app/router'
import { PKGlyph } from '@/components/shell/pk-glyph'
import { cn } from '@/lib/cn'
import { useRouteHistoryNav } from './use-route-history-nav'

export interface PKTopbarProps {
  screen: AppScreen
  onOpenPalette: () => void
  onBackupNow: () => void
  backupRunning: boolean
  archiveInitialized: boolean
  className?: string
  /**
   * macOS Overlay title bar only: when true the topbar header doubles as the
   * window-drag region (`data-tauri-drag-region`), so dragging the empty areas
   * of the bar moves the window — the native macOS affordance the Overlay
   * style otherwise removes. Off everywhere else (Windows/Linux/browser keep
   * native decorations), so the attribute is absent and the header is inert.
   *
   * Tauri v2 only starts a drag from a mousedown on an element that itself
   * carries the attribute; it never starts one from interactive children
   * (the back/forward buttons, the search trigger, the Backup CTA). Those
   * children deliberately do NOT carry the attribute, so they stay clickable.
   */
  titlebarDrag?: boolean
}

export function PKTopbar({
  screen,
  onOpenPalette,
  onBackupNow,
  backupRunning,
  archiveInitialized,
  className,
  titlebarDrag = false,
}: PKTopbarProps) {
  const { t } = useI18n()
  const { canGoBack, canGoForward, goBack, goForward, modifierLabel } =
    useRouteHistoryNav()
  const title = t(screen.titleKey)
  const subtitle = t(screen.subtitleKey)

  // Tauri v2 starts a window drag only from a mousedown on an element that
  // itself carries `data-tauri-drag-region`. We tag the header and its
  // non-interactive layout/text nodes so dragging the empty bar (and the page
  // title) moves the window, while every interactive leaf (nav buttons, search
  // trigger, Backup CTA) is left untagged and stays clickable. `undefined`
  // omits the attribute entirely off the macOS overlay platform.
  const drag = titlebarDrag ? '' : undefined

  return (
    <header
      className={cn(
        'border-border-light bg-paper flex h-[52px] shrink-0 items-center justify-between gap-3 border-b px-7',
        className,
      )}
      data-testid="pk-topbar"
      data-tauri-drag-region={drag}
    >
      <div
        className="flex min-w-0 items-center gap-3"
        data-tauri-drag-region={drag}
      >
        <div
          aria-label={t('navigation.routeHistory')}
          role="group"
          className="flex items-center gap-1"
          data-testid="pk-topbar-history"
        >
          <button
            type="button"
            aria-label={t('navigation.goBack')}
            title={`${t('navigation.goBack')} (${modifierLabel}[)`}
            onClick={goBack}
            disabled={!canGoBack}
            data-testid="pk-topbar-back"
            className={cn(
              'border-border-default text-ink-muted hover:border-ink-muted hover:text-ink',
              'flex h-7 w-7 items-center justify-center border font-mono text-[12px] leading-none transition-colors',
              'disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:border-border-default disabled:hover:text-ink-muted',
            )}
          >
            ←
          </button>
          <button
            type="button"
            aria-label={t('navigation.goForward')}
            title={`${t('navigation.goForward')} (${modifierLabel}])`}
            onClick={goForward}
            disabled={!canGoForward}
            data-testid="pk-topbar-forward"
            className={cn(
              'border-border-default text-ink-muted hover:border-ink-muted hover:text-ink',
              'flex h-7 w-7 items-center justify-center border font-mono text-[12px] leading-none transition-colors',
              'disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:border-border-default disabled:hover:text-ink-muted',
            )}
          >
            →
          </button>
        </div>
        <h1
          className="font-serif text-[18px] leading-none font-medium tracking-[-0.01em] text-ink"
          data-tauri-drag-region={drag}
        >
          {title}
        </h1>
        <span
          className="hidden truncate font-sans text-[12px] text-ink-faint sm:inline"
          data-tauri-drag-region={drag}
        >
          {subtitle}
        </span>
      </div>
      <div
        className="flex items-center gap-[10px]"
        data-tauri-drag-region={drag}
      >
        <button
          type="button"
          onClick={onOpenPalette}
          data-testid="pk-topbar-palette"
          className="border-border-default text-ink-faint hover:border-ink-muted hover:text-ink-muted flex items-center gap-2 border px-3 py-[6px] font-sans text-[12.5px] transition-colors min-w-[180px]"
        >
          <PKGlyph icon="search" size={14} />
          <span>{t('shell.findAPage')}</span>
          <kbd className="border-border-default bg-page text-ink-faint ml-auto border px-[5px] py-[1px] font-mono text-[10px]">
            {detectModifierLabel()}K
          </kbd>
        </button>
        <button
          type="button"
          onClick={onBackupNow}
          disabled={!archiveInitialized || backupRunning}
          className={cn(
            'border border-accent text-accent flex items-center gap-[6px] px-[14px] py-[6px] font-sans text-[12.5px] font-medium transition-colors',
            'hover:bg-accent-soft disabled:cursor-not-allowed disabled:opacity-60',
          )}
        >
          <PKGlyph icon={backupRunning ? 'pause' : 'play'} size={12} />
          {backupRunning ? t('shell.archiving') : t('navigation.backupNow')}
        </button>
      </div>
    </header>
  )
}

function detectModifierLabel(): string {
  if (typeof navigator === 'undefined') {
    return 'Ctrl+'
  }
  const platform = navigator.platform || ''
  if (/Mac|iPhone|iPod|iPad/.test(platform)) {
    return '⌘'
  }
  return 'Ctrl+'
}
