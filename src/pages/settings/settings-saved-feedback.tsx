/**
 * @file settings-saved-feedback.tsx
 * @description The reusable quiet "Saved" confirmation chip for the all-auto-save Settings page.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Render a small `aria-live="polite"` chip that announces a successful auto-save
 *   ("Saved" / "已保存" / "已儲存") while its `visible` prop is true, then fades out.
 *
 * ## Not responsible for
 * - Tracking the flash timing — that is `useSavedFeedback` in `use-saved-feedback.ts`.
 *   The hook lives in a separate module so this file stays component-only for fast-refresh.
 * - Performing the save; a section calls its existing `saveConfig` path and flashes
 *   only on success, so a failed write never shows "Saved".
 *
 * ## Performance notes
 * - Pure presentational. The fade is a CSS opacity transition that respects
 *   `prefers-reduced-motion`; no main-thread work.
 */

import { cn } from '@/lib/cn'
import { useI18n } from '../../lib/i18n'

export interface SettingsSavedChipProps {
  /** Whether the most recent successful save is still within its visible window. */
  visible: boolean
  className?: string
  testId?: string
}

/**
 * Renders the quiet "Saved" chip. It always occupies a polite live region so screen
 * readers announce the confirmation; when not visible it is fully transparent (and
 * `aria-hidden`) so it neither announces stale state nor reflows the row. Motion is a
 * short opacity fade that collapses to an instant show/hide under `prefers-reduced-motion`.
 */
export function SettingsSavedChip({
  visible,
  className,
  testId = 'settings-saved-chip',
}: SettingsSavedChipProps) {
  const { t } = useI18n()
  return (
    <span
      role="status"
      aria-live="polite"
      aria-hidden={visible ? undefined : true}
      data-testid={testId}
      data-visible={visible ? 'true' : 'false'}
      className={cn(
        'border-border-light text-ink-muted rounded-paper bg-paper inline-flex items-center gap-1.5 border px-2 py-0.5 font-mono text-[10.5px] tracking-[0.04em]',
        'transition-opacity duration-300 motion-reduce:transition-none',
        visible ? 'opacity-100' : 'opacity-0',
        className,
      )}
    >
      <span
        aria-hidden="true"
        className="bg-accent inline-block h-1.5 w-1.5 rounded-full"
      />
      {visible ? t('settings.savedConfirmation') : null}
    </span>
  )
}
