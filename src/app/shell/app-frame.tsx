/**
 * The window: nav rail on the left, the active screen in a rounded panel on
 * the right, plus the global shortcuts (⌘K palette, ⌘L lock, ⌘1…⌘9 or
 * Ctrl+1…9 for the spaces, ⌘[ / ⌘] or Alt+← / Alt+→ and the mouse's side
 * buttons for back and forward). On Windows and Linux the panel leaves a
 * strip of window background at the top for the window buttons, which the
 * app draws once for every screen (`window-chrome/`).
 * The rail follows the space of the page shown (`current-space.ts`), and
 * the panel fades when the space changes (`space-panel.tsx`).
 */
import { useCallback, useEffect, useState } from 'react'
import { Outlet, useLocation, useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import { backdropDragRegion } from '@/components/app/drag-region'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { subscribeToOpenCommandPalette } from '@/lib/ipc/desktop-events'
import { isMacOsHost } from '@/lib/runtime'
import { spaceKeyIndex, spacesOn } from '@/lib/spaces'
import { useSession } from '../session'
import { panelTopMargin } from '../window-chrome/frame'
import { CommandPalette } from './command-palette'
import { useSpaceFromUrl, useSwitchSpace } from './current-space'
import { NavRail } from './nav-rail'
import { SpacePanel } from './space-panel'
import { useHistorySteps } from './use-history-steps'

/** Typing keeps its own keys: Alt+← moves by word in a text field. */
function typing(target: EventTarget | null) {
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable ||
      target.tagName === 'INPUT' ||
      target.tagName === 'TEXTAREA' ||
      target.tagName === 'SELECT')
  )
}

export function AppFrame() {
  const { t } = useI18n()
  const session = useSession()
  const navigate = useNavigate()
  const [paletteOpen, setPaletteOpen] = useState(false)
  const lock = useCallback(() => {
    void session.lock().then((locked) => {
      if (locked) return
      toast(t('shell.lock.needsPasscode'))
      void navigate('/settings/security')
    })
  }, [session, navigate, t])

  useSpaceFromUrl(useLocation().pathname)
  const switchTo = useSwitchSpace()

  const steps = useHistorySteps()
  const { back, forward } = steps
  useEffect(() => {
    const mac = isMacOsHost()
    const onKey = (event: KeyboardEvent) => {
      if (typing(event.target)) return
      const step = mac
        ? event.metaKey && !event.altKey && !event.ctrlKey
          ? { '[': back, ']': forward }[event.key]
          : undefined
        : event.altKey && !event.metaKey && !event.ctrlKey
          ? { ArrowLeft: back, ArrowRight: forward }[event.key]
          : undefined
      if (step) {
        event.preventDefault()
        step()
      }
    }
    // The mouse's side buttons (3 back, 4 forward).
    const onMouse = (event: MouseEvent) => {
      if (event.button === 3) back()
      else if (event.button === 4) forward()
      else return
      event.preventDefault()
    }
    window.addEventListener('keydown', onKey)
    window.addEventListener('mouseup', onMouse)
    return () => {
      window.removeEventListener('keydown', onKey)
      window.removeEventListener('mouseup', onMouse)
    }
  }, [back, forward])

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey)) return
      const key = event.key.toLowerCase()
      if (key === 'k') {
        event.preventDefault()
        setPaletteOpen((open) => !open)
      } else if (key === 'l') {
        event.preventDefault()
        lock()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [lock])

  // ⌘1…⌘9 (Ctrl+1…9 on Windows and Linux): the nth space in the switcher.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const index = spaceKeyIndex(event)
      const target = index === null ? undefined : spacesOn()[index]
      if (!target) return
      event.preventDefault()
      switchTo(target)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [switchTo])

  // "Search history…" in the menu bar icon's menu.
  useEffect(() => {
    const unsubscribe = subscribeToOpenCommandPalette(() =>
      setPaletteOpen(true),
    )
    return () => void unsubscribe.then((stop) => stop())
  }, [])

  return (
    // The margin around the main panel is window background, and drags it like a title bar.
    <div
      {...backdropDragRegion()}
      className="relative flex h-full bg-window backdrop-blur-[40px] backdrop-saturate-[1.4] transition-[background-color] duration-300"
    >
      <NavRail onOpenPalette={() => setPaletteOpen(true)} onLock={lock} />
      <main
        className={cn(
          'mr-2.5 mb-2.5 flex min-w-0 flex-1 overflow-hidden rounded-xl border bg-panel',
          panelTopMargin(),
        )}
      >
        <SpacePanel>
          <Outlet />
        </SpacePanel>
      </main>
      <CommandPalette
        open={paletteOpen}
        onOpenChange={setPaletteOpen}
        onLock={lock}
      />
    </div>
  )
}
