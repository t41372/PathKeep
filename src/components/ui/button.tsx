/**
 * @file button.tsx
 * @description Paper-token-aligned Button primitive (shadcn "new-york" derivative).
 * @module components/ui
 *
 * ## Responsibilities
 * - Render the app's one Button primitive across six real-world clusters observed
 *   in an app-wide audit: filled primary CTA, neutral outline "secondary" (the
 *   workhorse), accent-bordered prominent action, borderless ghost, destructive,
 *   and link/text.
 * - Consume paper design tokens exclusively via Tailwind utilities (see
 *   `docs/design/design-tokens.md`) so no page needs a local color constant or
 *   raw `var(--x)` to theme a button correctly.
 * - Own the `loading` affordance (spinner overlay, `aria-busy`, non-interactive)
 *   so callers never hand-roll a spinner again. This holds for `asChild` too:
 *   native `disabled` is inert on a Slot child (e.g. an `<a>`), so the
 *   non-interactive guarantee is additionally enforced there via
 *   `aria-disabled`, a `pointer-events-none` class, and swallowing the click
 *   (`preventDefault` + `stopPropagation`) before it reaches the caller's
 *   handler or the child's native behavior (e.g. navigation).
 *
 * ## Not responsible for
 * - Route/page-specific button copy or icon choices.
 * - Translating the optional `loadingLabel` — callers own that string via their
 *   own `useI18n()` call (this primitive has no i18n context dependency).
 *
 * ## Dependencies
 * - `class-variance-authority` for the variant/size matrix, `radix-ui` Slot for
 *   `asChild`, `lucide-react` for the loading spinner glyph.
 *
 * ## Performance notes
 * - Pure className composition; no state, no effects. `loading` swaps a static
 *   overlay in place — never remounts the label — so idle→loading causes no
 *   layout reflow and no extra render passes.
 */

import * as React from 'react'
import { cva, type VariantProps } from 'class-variance-authority'
import { Slot } from 'radix-ui'
import { LoaderCircleIcon } from 'lucide-react'

import { cn } from '@/lib/cn'

const buttonVariants = cva(
  "inline-flex shrink-0 items-center justify-center rounded-paper font-sans font-medium whitespace-nowrap outline-none transition duration-150 focus-visible:ring-2 focus-visible:ring-accent disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
  {
    variants: {
      variant: {
        // Cluster 1 — filled primary CTA (e.g. paper-search-hero submit).
        // Label uses the fixed on-accent foreground (--color-primary-foreground,
        // #fdfcf9 in both themes) rather than the theme-flipping `text-paper`
        // surface token — `text-paper` inverts to near-black in dark mode, which
        // would put dark ink on the (theme-stable) slate accent fill.
        primary:
          'border border-accent bg-accent text-primary-foreground hover:opacity-90',
        // Cluster 2 — neutral outline "secondary", the dominant workhorse.
        outline:
          'border border-border-default bg-transparent text-ink-muted hover:border-ink-muted hover:bg-hover hover:text-ink',
        // Cluster 3 — accent-bordered prominent action (settings "primary action").
        accent:
          'border border-accent bg-transparent text-accent-text hover:bg-accent-soft',
        // Cluster 4 — borderless ghost.
        ghost:
          'border border-transparent bg-transparent text-ink-muted hover:bg-hover hover:text-ink',
        // Cluster 5 — destructive. Uses the error tokens (there is no --danger token).
        destructive:
          'border border-error bg-transparent text-error hover:bg-error-soft',
        // Cluster 6 — link / text button (activity-recent-toggle underline style).
        link: 'border border-transparent bg-transparent text-accent-text underline-offset-4 hover:underline',
      },
      size: {
        default: 'h-9 gap-1.5 px-3 py-1.5 text-xs has-[>svg]:px-2.5',
        sm: 'h-7 gap-1 px-2.5 py-1 text-xs has-[>svg]:px-2',
        lg: 'h-10 gap-2 px-4 py-2 text-sm has-[>svg]:px-3.5',
        icon: 'size-7 p-0',
        'icon-sm': "size-6 p-0 [&_svg:not([class*='size-'])]:size-3.5",
        'icon-lg': 'size-9 p-0',
      },
    },
    defaultVariants: {
      variant: 'outline',
      size: 'default',
    },
  },
)

function Button({
  className,
  variant = 'outline',
  size = 'default',
  asChild = false,
  loading = false,
  loadingLabel,
  disabled,
  onClick,
  children,
  ...props
}: React.ComponentProps<'button'> &
  VariantProps<typeof buttonVariants> & {
    /**
     * Renders the child element in place of a `<button>` (Radix Slot).
     * Because native `disabled` has no effect on a non-form child (e.g. an
     * `<a>`), `disabled`/`loading` non-interactivity is enforced here via
     * `aria-disabled`, a `pointer-events-none` class, and swallowing the
     * click before it reaches the child's native behavior or the caller's
     * `onClick`.
     */
    asChild?: boolean
    /**
     * Renders a reduced-motion-safe spinner overlay, marks the button
     * `aria-busy` + non-interactive, and keeps the idle label's width in the
     * layout via `opacity-0` (visually hidden but still in the accessibility
     * tree, unlike `visibility:hidden`) so there is no reflow between states
     * and the button never loses its accessible name. With `asChild`, the
     * spinner overlay is skipped (the child owns its own content), but
     * non-interactivity still holds — see `asChild` above.
     */
    loading?: boolean
    /**
     * Screen-reader-only text announced while `loading` is true. Callers own
     * translation (e.g. `t('common.loading')`) — this primitive never renders
     * hardcoded copy of its own.
     */
    loadingLabel?: string
  }) {
  const Comp = asChild ? Slot.Root : 'button'
  const isDisabled = Boolean(disabled || loading)

  // Native `disabled` only stops event dispatch on real form controls. When
  // `asChild` renders a non-form child (typically an `<a>`), the browser
  // still dispatches the click (and navigates) unless we swallow it
  // ourselves — this is what keeps a loading/disabled asChild button
  // non-interactive instead of a fully clickable, navigable control.
  const handleClick = React.useCallback(
    (event: React.MouseEvent<HTMLButtonElement>) => {
      if (isDisabled) {
        event.preventDefault()
        event.stopPropagation()
        return
      }
      onClick?.(event)
    },
    [isDisabled, onClick],
  )

  return (
    <Comp
      data-slot="button"
      data-variant={variant}
      data-size={size}
      data-loading={loading || undefined}
      aria-busy={loading || undefined}
      aria-disabled={asChild && isDisabled ? true : undefined}
      disabled={asChild ? undefined : isDisabled}
      onClick={handleClick}
      className={cn(
        buttonVariants({ variant, size, className }),
        asChild &&
          isDisabled &&
          'pointer-events-none cursor-not-allowed opacity-50',
      )}
      {...props}
    >
      {loading && !asChild ? (
        <span className="relative inline-flex items-center justify-center">
          <LoaderCircleIcon
            aria-hidden="true"
            data-slot="button-spinner"
            className="absolute inset-0 m-auto size-4 motion-safe:animate-spin"
          />
          {/*
            opacity-0 (not `invisible`/visibility:hidden) so this label stays
            in the accessibility tree — a loading button with no
            `loadingLabel` must still expose an accessible name.
          */}
          <span className="inline-flex items-center gap-1.5 opacity-0">
            {children}
          </span>
          {loadingLabel ? (
            <span className="sr-only">{loadingLabel}</span>
          ) : null}
        </span>
      ) : (
        children
      )}
    </Comp>
  )
}

export { Button, buttonVariants }
