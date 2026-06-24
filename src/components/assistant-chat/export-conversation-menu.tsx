/**
 * @file export-conversation-menu.tsx
 * @description The "Export" affordance for the AI assistant header — a small keyboard-accessible
 *              menu offering Markdown and JSON export of the current conversation.
 * @module components/assistant-chat
 *
 * ## Responsibilities
 * - Render an "Export" trigger button (disabled when there are no messages — honest) that opens a
 *   two-item menu: Markdown and JSON.
 * - Drive the menu's open/close + roving keyboard focus (ArrowUp/Down/Home/End/Escape, outside
 *   click), mirroring the shell's `ProfileSwitcher` menu grammar.
 * - Surface a transient, polite status (exporting / exported / failed) via an aria-live region so
 *   the result is announced without a heavy toast system. The actual serialize + save is owned by
 *   the caller's `onExport(format)` promise; this component only reflects its state.
 *
 * ## Not responsible for
 * - Building the Markdown / JSON document (that is `conversation-export.ts`).
 * - Picking a file path or writing to disk (the route owns the save-dialog → backend-write flow).
 *
 * ## Fluidity
 * - No work on the render path. The export promise runs off the main thread (async backend write);
 *   this component just toggles a small status enum.
 */

import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from 'react'
import { cn } from '@/lib/cn'
import { PKGlyph } from '@/components/shell/pk-glyph'
import type { ConversationExportFormat } from './conversation-export'

/** Localized copy for the export menu. */
export interface ExportConversationMenuCopy {
  /** Trigger button label (visible) + its aria-label base. */
  triggerLabel: string
  /** aria-label for the opened menu region. */
  menuLabel: string
  /** Markdown menu-item label. */
  markdownLabel: string
  /** JSON menu-item label. */
  jsonLabel: string
  /** Shown on the trigger while an export is running. */
  exportingLabel: string
  /** Announced (polite) on a successful export. */
  successLabel: string
  /** Announced (polite) on a failed export. */
  errorLabel: string
}

export interface ExportConversationMenuProps {
  copy: ExportConversationMenuCopy
  /** True when there is at least one message to export. */
  hasMessages: boolean
  /** Serialize + save the conversation in the chosen format; resolves true when a file was written. */
  onExport: (format: ConversationExportFormat) => Promise<boolean>
  /**
   * Test-id stem. Required (not optional) so the sub-element ids (`{testId}-trigger` / `-menu` /
   * `-markdown` / `-json` / `-status`) are always derivable without an unreachable fallback branch —
   * the only consumer is the assistant route, which always supplies one.
   */
  testId: string
}

type ExportStatus = 'idle' | 'exporting' | 'success' | 'error'

/** The two menu items, in render/focus order. */
const FORMATS: readonly ConversationExportFormat[] = ['markdown', 'json']

export function ExportConversationMenu({
  copy,
  hasMessages,
  onExport,
  testId,
}: ExportConversationMenuProps) {
  const [open, setOpen] = useState(false)
  const [status, setStatus] = useState<ExportStatus>('idle')
  const containerRef = useRef<HTMLDivElement | null>(null)
  const triggerRef = useRef<HTMLButtonElement | null>(null)
  const itemRefs = useRef<Array<HTMLButtonElement | null>>([])
  const statusTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Clear any pending status-reset timer on unmount so no late setState fires.
  useEffect(
    () => () => {
      if (statusTimerRef.current) clearTimeout(statusTimerRef.current)
    },
    [],
  )

  // Close on outside click / Escape while the menu is open (ProfileSwitcher grammar).
  useEffect(() => {
    if (!open) return
    const handlePointerDown = (event: MouseEvent) => {
      if (
        containerRef.current &&
        !containerRef.current.contains(event.target as Node)
      ) {
        setOpen(false)
      }
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false)
    }
    document.addEventListener('mousedown', handlePointerDown)
    document.addEventListener('keydown', handleKeyDown)
    return () => {
      document.removeEventListener('mousedown', handlePointerDown)
      document.removeEventListener('keydown', handleKeyDown)
    }
  }, [open])

  // Move focus to the first menu item when the menu opens.
  useEffect(() => {
    if (!open) return
    const frame = window.requestAnimationFrame(() =>
      itemRefs.current[0]?.focus(),
    )
    return () => window.cancelAnimationFrame(frame)
  }, [open])

  const focusItem = useCallback((index: number) => {
    itemRefs.current[index]?.focus()
  }, [])

  const runExport = useCallback(
    (format: ConversationExportFormat) => {
      setOpen(false)
      setStatus('exporting')
      if (statusTimerRef.current) clearTimeout(statusTimerRef.current)
      void onExport(format)
        .then((wrote) => {
          // A cancelled save dialog resolves false → return to idle silently (no false claim).
          setStatus(wrote ? 'success' : 'idle')
        })
        .catch(() => {
          setStatus('error')
        })
        .finally(() => {
          // Auto-clear the visible/announced result after a short, non-intrusive window.
          if (statusTimerRef.current) clearTimeout(statusTimerRef.current)
          statusTimerRef.current = setTimeout(() => setStatus('idle'), 2600)
        })
    },
    [onExport],
  )

  const handleItemKeyDown = useCallback(
    (event: ReactKeyboardEvent<HTMLButtonElement>, index: number) => {
      if (event.key === 'ArrowDown') {
        event.preventDefault()
        focusItem((index + 1) % FORMATS.length)
      } else if (event.key === 'ArrowUp') {
        event.preventDefault()
        focusItem((index - 1 + FORMATS.length) % FORMATS.length)
      } else if (event.key === 'Home') {
        event.preventDefault()
        focusItem(0)
      } else if (event.key === 'End') {
        event.preventDefault()
        focusItem(FORMATS.length - 1)
      } else if (event.key === 'Escape') {
        event.preventDefault()
        event.stopPropagation()
        setOpen(false)
        triggerRef.current?.focus()
      }
    },
    [focusItem],
  )

  const exporting = status === 'exporting'
  const triggerLabel = exporting ? copy.exportingLabel : copy.triggerLabel
  const itemLabels = [copy.markdownLabel, copy.jsonLabel] as const

  return (
    <div className="relative" ref={containerRef} data-testid={testId}>
      <button
        ref={triggerRef}
        type="button"
        disabled={!hasMessages || exporting}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={copy.triggerLabel}
        title={copy.triggerLabel}
        data-testid={`${testId}-trigger`}
        onClick={() => setOpen((current) => !current)}
        onKeyDown={(event) => {
          if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
            event.preventDefault()
            setOpen(true)
          }
        }}
        className={cn(
          'text-ink-secondary hover:text-accent hover:border-accent border-border-default',
          'rounded-paper bg-card-paper flex items-center gap-2 border px-3 py-1.5',
          'font-serif text-[13px] transition-colors duration-150',
          'disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:text-ink-secondary disabled:hover:border-border-default',
        )}
      >
        <PKGlyph icon="download" size={15} strokeWidth={1.8} />
        <span>{triggerLabel}</span>
      </button>

      {open ? (
        <div
          role="menu"
          aria-label={copy.menuLabel}
          data-testid={`${testId}-menu`}
          className={cn(
            'border-border-default bg-card-paper rounded-paper absolute right-0 z-20 mt-1',
            'flex min-w-[160px] flex-col border py-1 shadow-sm',
          )}
        >
          {FORMATS.map((format, index) => (
            <button
              key={format}
              ref={(element) => {
                itemRefs.current[index] = element
              }}
              type="button"
              role="menuitem"
              data-testid={`${testId}-${format}`}
              onClick={() => runExport(format)}
              onKeyDown={(event) => handleItemKeyDown(event, index)}
              className={cn(
                'text-ink hover:bg-paper-warm hover:text-accent flex items-center px-3 py-1.5',
                'text-left font-serif text-[13px] transition-colors duration-150',
              )}
            >
              {itemLabels[index]}
            </button>
          ))}
        </div>
      ) : null}

      {/* Polite, single-shot announcer for the export result. No heavy toast system. */}
      <span
        role="status"
        aria-live="polite"
        data-testid={`${testId}-status`}
        className="sr-only"
      >
        {status === 'success'
          ? copy.successLabel
          : status === 'error'
            ? copy.errorLabel
            : ''}
      </span>
    </div>
  )
}
