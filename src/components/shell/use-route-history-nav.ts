/**
 * Route history navigation policy hook for the v0.3 paper shell.
 *
 * ## Responsibilities
 * - Track an in-memory back-stack depth so `canGoBack` works under both
 *   BrowserRouter (production) and MemoryRouter (tests / browser preview)
 *   without leaning on `window.history.state.idx` (MemoryRouter does not
 *   write that field).
 * - Track whether `forward` is currently possible (browsers / react-router
 *   only expose forward state implicitly via `navigationType`).
 * - Bind global Cmd/Ctrl+[ and Cmd/Ctrl+] keyboard shortcuts to the
 *   same handlers as the topbar buttons so power users do not need to
 *   reach for the mouse.
 *
 * ## Not responsible for
 * - Rendering the back/forward chrome (PKTopbar owns that).
 * - Persisting history beyond the in-memory router stack — the browser
 *   history takes care of that lifecycle.
 *
 * ## Why this hook exists separately
 * - PKTopbar is unit-tested without react-router mounted in some
 *   snapshots; isolating the navigation policy keeps the component
 *   purely presentational while letting the hook own the side-effects
 *   (keyboard listener install/teardown).
 */

import { useCallback, useEffect, useRef, useState } from 'react'
import {
  NavigationType,
  useLocation,
  useNavigate,
  useNavigationType,
} from 'react-router-dom'

export interface RouteHistoryNav {
  canGoBack: boolean
  canGoForward: boolean
  goBack: () => void
  goForward: () => void
  /**
   * Localised modifier prefix for the keyboard shortcut hint, e.g.
   * `⌘` on macOS or `Ctrl+` on Linux/Windows. Mirrors the search
   * palette hint so the chrome stays internally consistent.
   */
  modifierLabel: string
}

const isMacLike = (): boolean => {
  if (typeof navigator === 'undefined') return false
  // navigator.platform is deprecated but still the most reliable signal
  // until userAgentData rolls out everywhere. Fallback to UA sniff.
  const platform = navigator.platform || ''
  if (/Mac|iPhone|iPod|iPad/.test(platform)) return true
  return /Mac OS X/.test(navigator.userAgent || '')
}

const modifierLabelForPlatform = (): string => (isMacLike() ? '⌘' : 'Ctrl+')

const shortcutMatches = (event: KeyboardEvent, key: '[' | ']'): boolean => {
  if (event.key !== key) return false
  // Avoid hijacking shortcuts the OS / browser owns (e.g. window switch
  // shortcuts on Linux use Alt/Super). Match either Meta (Cmd) on macOS
  // or Ctrl elsewhere; never both, never Alt/Shift on top.
  if (event.altKey) return false
  if (event.shiftKey) return false
  if (isMacLike()) {
    return event.metaKey && !event.ctrlKey
  }
  return event.ctrlKey && !event.metaKey
}

const isEditableTarget = (target: EventTarget | null): boolean => {
  if (!target || !(target instanceof HTMLElement)) return false
  const tag = target.tagName
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return true
  // jsdom does not always compute `isContentEditable` from the parent
  // chain, so fall through to the rendered attribute. Both production
  // browsers and jsdom honour the attribute itself, which is what we
  // care about: "did the author opt this node in to text input?"
  if (target.isContentEditable) return true
  const editableAttr = target.getAttribute('contenteditable')
  if (
    editableAttr !== null &&
    editableAttr !== 'false' &&
    editableAttr !== 'inherit'
  ) {
    return true
  }
  return false
}

export function useRouteHistoryNav(): RouteHistoryNav {
  const navigate = useNavigate()
  const navigationType = useNavigationType()
  const location = useLocation()
  // Index into the back-stack maintained locally. We can't trust
  // `window.history.state.idx` because MemoryRouter doesn't write it,
  // and the BrowserRouter's value resets on a hard refresh which would
  // briefly disable the back button even when the browser still has
  // entries to step back to. Counting Push / Pop transitions locally is
  // honest about "what we've seen in this React tree" — which is
  // exactly what the topbar control should reflect.
  const [stackIndex, setStackIndex] = useState(0)
  const [forwardAvailable, setForwardAvailable] = useState(false)
  // Track the last location key we observed so we can attribute the
  // current `navigationType` to the right transition. Without this guard
  // the initial mount (always `Pop` per react-router) would underflow
  // the stack to -1 → 0.
  const lastKeyRef = useRef<string | null>(null)
  // Intent ring: react-router emits NavigationType.Pop for both
  // history.go(-1) AND history.go(+1) — there is no way to tell from
  // navigationType alone whether the Pop came from a back step or a
  // forward step. goForward writes 'forward' here before calling
  // navigate(1); the effect reads the intent on the very next Pop
  // and increments the stack accordingly, then clears the ref. Any
  // other Pop (browser back button, goBack, in-app `<a>` with
  // back-relative href) sees an empty intent and falls back to the
  // back semantics. Code-review §4 regression: the original effect
  // unconditionally treated Pop as back, so goForward left both
  // arrows disabled.
  const navIntentRef = useRef<'forward' | null>(null)

  useEffect(() => {
    if (lastKeyRef.current === location.key) return
    if (lastKeyRef.current === null) {
      // First render: align our counter with the router but do not
      // attribute a delta — the user has not navigated yet.
      lastKeyRef.current = location.key
      return
    }
    lastKeyRef.current = location.key
    // This block intentionally calls setState inside the effect: the
    // hook is integrating router navigation events (an external source
    // of truth driven by user gestures) into the local back/forward
    // counter. There is no "derived from props" path that captures
    // both Push (counter +1) and Pop (counter -1) honestly without
    // duplicating the lastKey tracking — so the side effect must
    // fire here. Cascading renders are bounded because both setters
    // use the functional updater form and only fire when location.key
    // actually changes (guarded above by the lastKeyRef check).
    if (navigationType === NavigationType.Push) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setStackIndex((index) => index + 1)
      // A Push wipes any in-flight forward branch, mirroring browser
      // behaviour. Otherwise a back-then-link-click would still leave
      // the forward arrow lit.
      setForwardAvailable(false)
      navIntentRef.current = null
    } else if (navigationType === NavigationType.Pop) {
      if (navIntentRef.current === 'forward') {
        // goForward-triggered Pop: walk DOWN the forward branch,
        // increment the counter, and leave forwardAvailable cleared
        // by goForward (no further setState here).
        navIntentRef.current = null
        setStackIndex((index) => index + 1)
      } else {
        // Browser back / goBack-triggered Pop: walk up the back
        // branch.
        setStackIndex((index) => Math.max(0, index - 1))
      }
    }
    // NavigationType.Replace intentionally does not move the counter —
    // a redirect / canonicalisation should not arm the back button.
  }, [location.key, navigationType])

  const canGoBack = stackIndex > 0
  const canGoForward = forwardAvailable

  const goBack = useCallback(() => {
    if (stackIndex <= 0) return
    setForwardAvailable(true)
    navIntentRef.current = null
    void navigate(-1)
  }, [navigate, stackIndex])

  const goForward = useCallback(() => {
    if (!forwardAvailable) return
    setForwardAvailable(false)
    navIntentRef.current = 'forward'
    void navigate(1)
  }, [forwardAvailable, navigate])

  useEffect(() => {
    function handleKey(event: KeyboardEvent) {
      if (isEditableTarget(event.target)) return
      if (shortcutMatches(event, '[')) {
        event.preventDefault()
        goBack()
        return
      }
      if (shortcutMatches(event, ']')) {
        event.preventDefault()
        goForward()
      }
    }
    document.addEventListener('keydown', handleKey)
    return () => document.removeEventListener('keydown', handleKey)
  }, [goBack, goForward])

  return {
    canGoBack,
    canGoForward,
    goBack,
    goForward,
    modifierLabel: modifierLabelForPlatform(),
  }
}
