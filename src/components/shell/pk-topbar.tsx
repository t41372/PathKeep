/**
 * Paper-redesign topbar: serif page title + ⌘K search trigger + Backup CTA.
 *
 * Why this file exists:
 * - The topbar is read first on every route render; getting its serif sizing
 *   right is what makes the app feel like a book rather than a SaaS dashboard.
 *
 * Not responsible for:
 * - Search palette rendering (delegates to PKSearchPalette via onOpenPalette).
 * - Running backup (delegates to onBackupNow).
 */

import { useI18n } from '@/lib/i18n/hooks'
import type { AppScreen } from '@/app/router'
import { PKGlyph } from '@/components/shell/pk-glyph'
import { cn } from '@/lib/cn'

export interface PKTopbarProps {
  screen: AppScreen
  onOpenPalette: () => void
  onBackupNow: () => void
  backupRunning: boolean
  archiveInitialized: boolean
  className?: string
}

export function PKTopbar({
  screen,
  onOpenPalette,
  onBackupNow,
  backupRunning,
  archiveInitialized,
  className,
}: PKTopbarProps) {
  const { t } = useI18n()
  const title = t(screen.titleKey)
  const subtitle = t(screen.subtitleKey)

  return (
    <header
      className={cn(
        'border-border-light bg-paper flex h-[52px] shrink-0 items-center justify-between gap-3 border-b px-7',
        className,
      )}
      data-testid="pk-topbar"
    >
      <div className="flex min-w-0 items-baseline gap-3">
        <h1 className="font-serif text-[18px] leading-none font-medium tracking-[-0.01em] text-ink">
          {title}
        </h1>
        <span className="hidden truncate font-sans text-[12px] text-ink-faint sm:inline">
          {subtitle}
        </span>
      </div>
      <div className="flex items-center gap-[10px]">
        <button
          type="button"
          onClick={onOpenPalette}
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
