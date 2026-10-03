/**
 * The window: nav rail on the left, the active screen in a rounded panel on
 * the right, plus the global shortcuts (⌘K palette, ⌘L lock).
 */
import { useEffect, useState } from 'react'
import { Outlet } from 'react-router-dom'
import { useSession } from '../session'
import { CommandPalette } from './command-palette'
import { NavRail } from './nav-rail'

export function AppFrame() {
  const session = useSession()
  const [paletteOpen, setPaletteOpen] = useState(false)
  const lock = () => void session.lock()

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey)) return
      const key = event.key.toLowerCase()
      if (key === 'k') {
        event.preventDefault()
        setPaletteOpen((open) => !open)
      } else if (key === 'l') {
        event.preventDefault()
        void session.lock()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [session])

  return (
    <div className="flex h-full bg-window backdrop-blur-[40px] backdrop-saturate-[1.4]">
      <NavRail onOpenPalette={() => setPaletteOpen(true)} onLock={lock} />
      <main className="my-2.5 mr-2.5 flex min-w-0 flex-1 overflow-hidden rounded-xl border bg-panel">
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
