/**
 * @file ambient-task-announcer.tsx
 * @description Always-mounted screen-reader live region that announces when shell background work
 *   APPEARS or DISAPPEARS — the a11y presence signal for the ambient task bar.
 *
 * ## Responsibilities
 * - Announce a single, presence-based transition: false→true ("started") and true→false ("ended").
 * - Stay mounted independently of the ambient bar (which unmounts when idle) so the disappearance
 *   transition can still be announced after the bar is gone.
 *
 * ## Not responsible for
 * - Announcing per-tick progress: the region deliberately reacts only to the boolean `active`
 *   transition, never to progress values, so assistive tech is not spammed on every poll tick.
 * - Rendering the visible strip (see components/primitives/ambient-task-bar.tsx) or selecting tasks
 *   (see app/shell-ambient-tasks.ts).
 * - Localizing copy — it receives already-localized `startedLabel` / `endedLabel` strings.
 */

import { useState } from 'react'

export interface AmbientTaskAnnouncerProps {
  /** Whether any background work is currently active. */
  active: boolean
  /** Localized message announced on the false→true transition. */
  startedLabel: string
  /** Localized message announced on the true→false transition. */
  endedLabel: string
}

export function AmbientTaskAnnouncer({
  active,
  startedLabel,
  endedLabel,
}: AmbientTaskAnnouncerProps) {
  const [message, setMessage] = useState('')
  // Remember the previously-seen presence so we speak ONLY on the boolean
  // transition. Seeded with the initial `active` so a component that mounts
  // while work is already running does NOT announce on mount, and a re-render
  // that leaves `active` unchanged (a progress tick) is a no-op → no SR spam.
  // This is React's recommended "adjust state during render" pattern
  // (react.dev/reference/react/useState#storing-information-from-previous-renders),
  // preferred over a setState-in-effect that would trigger a cascading render.
  const [prevActive, setPrevActive] = useState(active)
  if (prevActive !== active) {
    setPrevActive(active)
    setMessage(active ? startedLabel : endedLabel)
  }

  return (
    <div
      className="sr-only"
      role="status"
      aria-live="polite"
      data-testid="ambient-task-announcer"
    >
      {message}
    </div>
  )
}
