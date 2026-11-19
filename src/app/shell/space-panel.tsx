/**
 * The content panel's fade when the space changes: the old space's page
 * fades out while the rail slides, and the next space's page fades in once
 * it arrives (`current-space.ts` asks for it after the slide). Moving
 * between a space's own pages, or to Ask or Settings and back, does not
 * fade, and neither does the window opening. An address of a kind that is
 * off counts like a shared screen: the rail stays where it was.
 *
 * Responsibilities: the fade. Not responsible for: the rail's slide
 * (`space-slide.tsx`) or which space is current.
 */
import { motion, useReducedMotion } from 'motion/react'
import { useState, type ReactNode } from 'react'
import { useLocation } from 'react-router-dom'
import { spaceOfPath, useSpacesOn } from '@/lib/spaces'
import { useCurrentSpace } from './current-space'

export function SpacePanel({ children }: { children: ReactNode }) {
  const space = useCurrentSpace()
  const still = !!useReducedMotion()
  const on = useSpacesOn()
  const id = spaceOfPath(useLocation().pathname)
  const pathSpace = on.some((item) => item.id === id) ? id : null
  // A shared screen counts as the space it was opened from.
  const [pageSpace, setPageSpace] = useState(pathSpace ?? space.id)
  if (pathSpace && pathSpace !== pageSpace) setPageSpace(pathSpace)
  // A space turned off meanwhile is no longer the one it was opened from.
  const shown =
    pathSpace ??
    (on.some((item) => item.id === pageSpace) ? pageSpace : space.id)
  // The space changed and its page has not arrived yet.
  const leaving = shown !== space.id
  const [firstShown] = useState(shown)
  const [switched, setSwitched] = useState(false)
  if (!switched && shown !== firstShown) setSwitched(true)

  return (
    <motion.div
      key={shown}
      initial={switched ? { opacity: 0 } : false}
      animate={{ opacity: leaving ? 0 : 1 }}
      transition={{
        duration: leaving ? 0.1 : still ? 0.15 : 0.22,
        delay: leaving ? 0 : 0.04,
      }}
      className="flex min-w-0 flex-1"
    >
      {children}
    </motion.div>
  )
}
