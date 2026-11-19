/**
 * The space the rail shows, and switching to another.
 *
 * Responsibilities: on a space's own page the space comes from the URL, so a
 * link, a search result or ⌘K into another space switches with it; on a
 * shared screen (Ask, Settings), or on an address of a kind that is off
 * (which shows that it is off), the last space stays. A space whose kind
 * goes off while it is current gives way to the first space that is on.
 * The current space is remembered on this computer (`readLastSpace`) and
 * put on `<html>` as `data-space`, with `data-space-hue` and `--space-h`
 * for a space that takes its own colours (`src/index.css`).
 *
 * A switch moves the rail first: the space changes at once, its items slide
 * (`space-slide.tsx`), and the next space's page is asked for only once the
 * old items have left, so a heavy page cannot hold the slide back.
 *
 * Not responsible for: the rail, the menu or the shortcut keys
 * (`nav-rail.tsx`, `space-switcher.tsx`, `app-frame.tsx`).
 */
import {
  startTransition,
  useCallback,
  useLayoutEffect,
  useState,
  useSyncExternalStore,
} from 'react'
import { flushSync } from 'react-dom'
import { useNavigate } from 'react-router-dom'
import {
  readLastSpace,
  rememberSpace,
  spaceOfPath,
  spacesOn,
  spaceStartPath,
  useSpacesOn,
  type Space,
} from '@/lib/spaces'

/** How long the old space's items take to slide out (`space-slide.tsx`). */
export const SPACE_LEAVE_MS = 170

let current: Space = readLastSpace()
const listeners = new Set<() => void>()

function paint(space: Space) {
  const html = document.documentElement
  html.dataset.space = space.id
  if (space.baseTokens) {
    delete html.dataset.spaceHue
    html.style.removeProperty('--space-h')
  } else {
    html.dataset.spaceHue = ''
    html.style.setProperty('--space-h', String(space.hue))
  }
}
paint(current)

function setCurrent(space: Space) {
  if (space.id === current.id) return
  current = space
  rememberSpace(space.id)
  paint(space)
  for (const listener of listeners) listener()
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => void listeners.delete(listener)
}

export function useCurrentSpace(): Space {
  return useSyncExternalStore(subscribe, () => current)
}

/**
 * Follows the URL into the pages of a space that is on. Mounted once, by
 * the frame.
 */
export function useSpaceFromUrl(pathname: string) {
  const id = spaceOfPath(pathname)
  const on = useSpacesOn()
  // Before paint, so the rail and the page never show two spaces at once.
  // Only a change of route (or of which spaces are on) counts: a switch sets
  // the space first and changes the route a moment later, and must not be
  // undone in between.
  useLayoutEffect(() => {
    const space = on.find((item) => item.id === id)
    if (space) setCurrent(space)
    else if (!on.some((item) => item.id === current.id)) setCurrent(on[0])
  }, [id, on])
}

/** The path the window shows now; both of PathKeep's routers use the hash. */
function currentPathname() {
  return window.location.hash.replace(/^#/, '').split('?')[0]
}

let pendingPage: ReturnType<typeof setTimeout> | undefined

/**
 * Switches to a space, opening its first page. From a shared screen whose
 * rail already shows that space it just opens the page; on one of its own
 * pages it does nothing.
 */
export function useSwitchSpace() {
  const navigate = useNavigate()
  return useCallback(
    (space: Space) => {
      // A second switch before the first page arrived replaces it, so quick
      // ⌘1 ⌘2 presses end on the last space pressed.
      clearTimeout(pendingPage)
      if (spaceOfPath(currentPathname()) === space.id) return
      const to = spaceStartPath(space)
      if (space.id === current.id) {
        void navigate(to)
        return
      }
      flushSync(() => setCurrent(space))
      pendingPage = setTimeout(
        () => startTransition(() => void navigate(to)),
        SPACE_LEAVE_MS,
      )
    },
    [navigate],
  )
}

/**
 * Which way the rail slides: 1 when `space` comes after the one shown
 * before it in the switcher's order, -1 when before.
 */
export function useSlideDirection(space: Space) {
  const [seen, setSeen] = useState({ id: space.id, dir: 1 })
  if (seen.id === space.id) return seen.dir
  const order = spacesOn().map((item) => item.id)
  const dir = order.indexOf(space.id) >= order.indexOf(seen.id) ? 1 : -1
  setSeen({ id: space.id, dir })
  return dir
}
