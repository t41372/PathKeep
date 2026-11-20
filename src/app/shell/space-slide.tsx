/**
 * The rail's part that belongs to a space (the switcher's face and the
 * space's pages) moving when the space changes: the old items slide out
 * sideways and the new ones slide in from the other side, one after another,
 * in the direction of the switch (a space later in the switcher's order
 * moves the content left). With "reduce motion" on it is only a cross-fade.
 *
 * Responsibilities: the animation. Not responsible for: what the items
 * are, which way the switch went, or holding the next space's page back
 * until the old items have left (`current-space.ts`).
 *
 * Only `transform` and `opacity` move, as strings motion hands to the
 * browser's own animations, so the slide keeps going while the next page
 * renders on the main thread.
 */
import {
  AnimatePresence,
  motion,
  useReducedMotion,
  type Variants,
} from 'motion/react'
import { Children, type ReactNode } from 'react'
import { cn } from '@/lib/cn'

const DISTANCE = 26
const ease = [0.22, 1, 0.36, 1] as const

function itemVariants(still: boolean): Variants {
  const shift = (px: number) => `translateX(${still ? 0 : px}px)`
  return {
    enter: (dir: number) => ({ transform: shift(dir * DISTANCE), opacity: 0 }),
    center: {
      transform: shift(0),
      opacity: 1,
      transition: { duration: 0.26, ease },
    },
    exit: (dir: number) => ({
      transform: shift(-dir * DISTANCE),
      opacity: 0,
      transition: { duration: 0.16, ease: 'easeIn' },
    }),
  }
}

const groupVariants: Variants = {
  enter: {},
  center: { transition: { staggerChildren: 0.035, delayChildren: 0.04 } },
  exit: { transition: { staggerChildren: 0.02 } },
}

/** Each child slides on its own, a little after the one before it. */
export function SpaceSlide({
  id,
  dir,
  className,
  children,
}: {
  /** The space shown; a new one slides the children out and in. */
  id: string
  dir: number
  className?: string
  children: ReactNode
}) {
  const still = !!useReducedMotion()
  const variants = itemVariants(still)
  return (
    <div className="relative">
      <AnimatePresence initial={false} mode="popLayout" custom={dir}>
        <motion.div
          key={id}
          // Which space's items these are, while two sets pass each other.
          data-space-slide={id}
          custom={dir}
          variants={groupVariants}
          initial="enter"
          animate="center"
          exit="exit"
          className={cn('flex flex-col items-center', className)}
        >
          {Children.map(children, (child) => (
            <motion.div custom={dir} variants={variants}>
              {child}
            </motion.div>
          ))}
        </motion.div>
      </AnimatePresence>
    </div>
  )
}
