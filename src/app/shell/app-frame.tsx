/**
 * The window: nav rail on the left, the active screen in a rounded panel on
 * the right, plus the global shortcuts (⌘K palette, ⌘L lock).
 */
import { useCallback, useEffect, useState } from 'react'
import { Outlet, useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import { useI18n } from '@/lib/i18n'
import { subscribeToOpenCommandPalette } from '@/lib/ipc/desktop-events'
import { useSession } from '../session'
import { CommandPalette } from './command-palette'
import { NavRail } from './nav-rail'

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
