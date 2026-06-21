/**
 * StarToggle — the single star (favorites / 加星) affordance shared by every
 * Explorer surface (list rows, contact cards, the detail panel, search
 * results, assistant evidence, intelligence entity rows) and the Starred hub.
 *
 * Why this file exists:
 * - One toggle, one visual identity. A filled accent star means "starred"; an
 *   outline star means "not starred". The component owns the glyph, the
 *   `aria-pressed` semantics, and the `S` keyboard shortcut so callers only
 *   pass `starred` + `onToggle` + copy.
 *
 * Behaviour contract:
 * - Optimistic: the caller flips its own state on `onToggle`; this component is
 *   purely presentational and never awaits the backend.
 * - No layout shift: the button always occupies its box. When not starred it is
 *   dimmed and only reaches full opacity on hover/focus or when its row is
 *   selected (`alwaysVisible`); when starred it is always fully visible. The
 *   element is never removed from the layout, so rows never reflow.
 * - `S` toggles when the button (or, via `alwaysVisible`, the selected row) is
 *   focused — handled by the caller forwarding key events, or by focusing the
 *   button directly.
 */

import { cn } from '@/lib/cn'

export interface StarToggleProps {
  /** Whether the entity is currently starred (drives fill + aria-pressed). */
  starred: boolean
  /** Called when the user activates the toggle. Caller owns optimistic state. */
  onToggle: () => void
  /** Accessible label when NOT starred (e.g. "Star"). */
  starLabel: string
  /** Accessible label when starred (e.g. "Unstar"). */
  unstarLabel: string
  /**
   * Keep the outline star fully visible even when not starred (e.g. the row is
   * selected, or the detail panel where the action bar is always shown).
   * Defaults to false: unstarred toggles hover-reveal so dense lists stay calm.
   */
  alwaysVisible?: boolean
  /** Icon edge length in px. Defaults to 16 to sit inside dense rows. */
  size?: number
  /**
   * State words announced by the visually-hidden polite live region after a
   * toggle, e.g. `{ starred: 'Starred', unstarred: 'Unstarred' }`. Falls back
   * to the action labels when omitted so the announcement is never empty.
   */
  statusLabel?: { starred: string; unstarred: string }
  className?: string
  testId?: string
}

/**
 * Renders the star button. The SVG is inlined (rather than going through
 * `PKGlyph`) so the fill can switch between `none` (outline) and `currentColor`
 * (filled accent) without fighting the shared glyph wrapper's `fill="none"`.
 */
export function StarToggle({
  starred,
  onToggle,
  starLabel,
  unstarLabel,
  alwaysVisible = false,
  size = 16,
  statusLabel,
  className,
  testId,
}: StarToggleProps) {
  const label = starred ? unstarLabel : starLabel
  // The button's `aria-label` describes the *action* ("Star" / "Unstar"); a
  // separate visually-hidden polite live region announces the resulting *state*
  // ("Starred" / "Unstarred") after a toggle, so a screen-reader user hears the
  // change confirmed without re-reading the whole row.
  const stateAnnouncement = starred
    ? (statusLabel?.starred ?? unstarLabel)
    : (statusLabel?.unstarred ?? starLabel)
  return (
    <span className="contents">
      <button
        type="button"
        aria-pressed={starred}
        aria-label={label}
        title={label}
        data-testid={testId}
        data-starred={starred ? 'true' : 'false'}
        onClick={(event) => {
          // Star toggles must never bubble into the row's own select/open
          // handler — clicking the star should not also open the detail panel.
          event.stopPropagation()
          onToggle()
        }}
        onKeyDown={(event) => {
          if (event.key === 's' || event.key === 'S') {
            event.preventDefault()
            event.stopPropagation()
            onToggle()
          }
        }}
        className={cn(
          'rounded-paper inline-flex h-6 w-6 shrink-0 items-center justify-center',
          'transition-[color,opacity] duration-150 outline-none',
          'focus-visible:ring-1 focus-visible:ring-accent',
          starred
            ? 'text-accent opacity-100'
            : cn(
                'text-ink-faint hover:text-accent',
                // Reveal on hover/focus, or always when the row is selected.
                alwaysVisible
                  ? 'opacity-100'
                  : 'opacity-0 group-hover:opacity-100 focus-visible:opacity-100 data-[starred=true]:opacity-100',
              ),
          className,
        )}
      >
        <svg
          viewBox="0 0 24 24"
          width={size}
          height={size}
          fill={starred ? 'currentColor' : 'none'}
          stroke="currentColor"
          strokeWidth={1.8}
          strokeLinecap="round"
          strokeLinejoin="round"
          className="block"
          aria-hidden="true"
        >
          <path d="M12 3.5l2.6 5.3 5.9.9-4.25 4.15 1 5.85L12 17.9l-5.25 2.65 1-5.85L3.5 9.7l5.9-.9z" />
        </svg>
      </button>
      <span
        role="status"
        aria-live="polite"
        className="sr-only"
        data-testid={testId ? `${testId}-status` : undefined}
      >
        {stateAnnouncement}
      </span>
    </span>
  )
}
