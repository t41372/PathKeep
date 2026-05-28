/**
 * Class-name composition helper used by every shadcn primitive and any
 * Tailwind-using component in PathKeep.
 *
 * Why this file exists:
 * - shadcn primitives expect `cn` to be importable from `@/lib/cn` (we configure
 *   that path in components.json).
 * - clsx handles conditional class composition. tailwind-merge resolves
 *   conflicting Tailwind utilities so the last one wins predictably.
 */

import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

/**
 * Merge an arbitrary set of class-name inputs into a single deduplicated string.
 *
 * Accepts the same inputs as clsx: strings, arrays, falsy values, and objects
 * whose keys become class names when the value is truthy.
 */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs))
}
