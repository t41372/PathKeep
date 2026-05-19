/**
 * Inline PathKeep brand mark.
 *
 * Why this file exists:
 * - The orange box + filled tiles + gray lines glyph is the single most
 *   recognizable PathKeep asset. Inlining the SVG avoids a network hop and
 *   lets it pick up theme colors via currentColor or explicit fill props.
 *
 * The shape mirrors src/assets/pathkeep-mark.svg exactly (verified against
 * the design package's pathkeep-mark.svg). Do not redraw without updating
 * both copies.
 */

import { cn } from '@/lib/cn'

export interface PKBrandMarkProps {
  size?: number
  className?: string
}

export function PKBrandMark({ size = 30, className }: PKBrandMarkProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 512 512"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={cn('block shrink-0', className)}
      aria-label="PathKeep"
      role="img"
    >
      <rect
        x="86"
        y="86"
        width="340"
        height="340"
        stroke="#FF7B33"
        strokeWidth="24"
      />
      <rect x="166" y="178" width="54" height="54" fill="#FF7B33" />
      <rect x="166" y="280" width="54" height="54" fill="#FF7B33" />
      <path d="M193 232V280" stroke="#FF7B33" strokeWidth="18" />
      <path
        d="M262 205H350M262 256H350M262 307H322"
        stroke="#8B8B8B"
        strokeWidth="18"
        strokeLinecap="square"
      />
    </svg>
  )
}
