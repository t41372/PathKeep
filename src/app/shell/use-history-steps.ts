/**
 * Back and forward through the screens visited, for the arrows at the top of
 * the nav rail and their shortcuts.
 *
 * Responsibilities: whether there is a step back or forward, and taking it.
 * Filters and selections replace the current entry (`useSearchParamsUpdate`),
 * so the steps are screens and drill-ins, not every click.
 * Not responsible for: the buttons or the keys (`nav-rail.tsx`,
 * `app-frame.tsx`).
 */
import { useCallback, useState } from 'react'
import {
  NavigationType,
  useLocation,
  useNavigate,
  useNavigationType,
} from 'react-router-dom'

/** The router keeps each entry's position in `history.state.idx`. */
function position() {
  const state = window.history.state as { idx?: number } | null
  return state?.idx ?? 0
}

export function useHistorySteps() {
  const location = useLocation()
  const type = useNavigationType()
  const navigate = useNavigate()
  const here = position()
  // The furthest position forward still reachable: a new screen drops what
  // was ahead, going back keeps it. Updated as each navigation renders.
  const [track, setTrack] = useState({ key: location.key, furthest: here })
  if (track.key !== location.key) {
    setTrack({
      key: location.key,
      furthest:
        type === NavigationType.Push ? here : Math.max(track.furthest, here),
    })
  }
  const back = useCallback(() => void navigate(-1), [navigate])
  const forward = useCallback(() => void navigate(1), [navigate])
  return {
    canBack: here > 0,
    canForward: here < Math.max(track.furthest, here),
    back,
    forward,
  }
}
