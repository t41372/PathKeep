/**
 * The window: nav rail on the left, the active screen in a rounded panel on
 * the right, plus the global shortcuts (⌘K palette, ⌘L lock, ⌘[ / ⌘] or
 * Alt+← / Alt+→ and the mouse's side buttons for back and forward). On
 * Windows and Linux it also draws the window buttons at the top right, over
 * a strip of window background above the panel.
 */
import { useCallback, useEffect, useState } from 'react'
import { Outlet, useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import { backdropDragRegion } from '@/components/app/drag-region'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { subscribeToOpenCommandPalette } from '@/lib/ipc/desktop-events'
import { drawsWindowControls, isMacOsHost } from '@/lib/runtime'
import { useSession } from '../session'
import { CommandPalette } from './command-palette'
import { NavRail } from './nav-rail'
import { useHistorySteps } from './use-history-steps'
import { WindowControls } from './window-controls'

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
  const controls = drawsWindowControls()
  const lock = useCallback(() => {
    void session.lock().then((locked) => {
      if (locked) return
      toast(t('shell.lock.needsPasscode'))
      void navigate('/settings/security')
    })
  }, [session, navigate, t])

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
      className="relative flex h-full bg-window backdrop-blur-[40px] backdrop-saturate-[1.4]"
    >
      {controls && <WindowControls />}
      <NavRail onOpenPalette={() => setPaletteOpen(true)} onLock={lock} />
      <main
        className={cn(
          'mr-2.5 mb-2.5 flex min-w-0 flex-1 overflow-hidden rounded-xl border bg-panel',
          // Windows and Linux: room above the panel for the window buttons.
          controls ? 'mt-8' : 'mt-2.5',
        )}
      >
        <Outlet />
      </main>
      <CommandPalette
        open={paletteOpen}
        onOpenChange={setPaletteOpen}
        onLock={lock}
      />
    </div>
  )
}
