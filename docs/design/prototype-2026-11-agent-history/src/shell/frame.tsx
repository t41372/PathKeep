/**
 * The window: rail and the rounded panel. Two shells to compare:
 * - spaces (round 3): a switcher picks the data type; the rail always lists
 *   that type's pages; the window takes the type's tint.
 * - types (round 2): one rail item per type, the type's pages as tabs on top.
 * Screens don't know which shell drew them.
 */
import { motion } from 'motion/react'
import { useEffect, useState } from 'react'
import { Outlet, useLocation, useNavigate } from 'react-router-dom'
import { Toaster } from 'sonner'
import { Button } from '@/components/ui/button'
import { kindById, kinds, spaceOfPath, type Kind } from '~/kinds'
import { useProto } from '~/proto-state'
import { EnableAgentDialog } from '~/components/enable-agent'
import { Knobs } from './knobs'
import { Palette } from './palette'
import { spaceKeyIndex } from './space-key'
import { SpaceRail, useSwitchSpace } from './space-rail'

function kindForPath(pathname: string): Kind | undefined {
  return kinds.find((k) => k.pages.some((p) => pathname === p.path || pathname.startsWith(p.path + '/') || (k.real && pathname.startsWith('/' + p.path.split('/')[1] + '/'))))
}

/** A type that is off: its archive is kept, the pages just aren't in the rail. */
function KindOff({ kind }: { kind: Kind }) {
  const navigate = useNavigate()
  return (
    <div className="flex flex-1 items-center justify-center p-10">
      <div className="flex max-w-[420px] flex-col items-center gap-3 text-center">
        <kind.icon className="size-8 text-muted-foreground" strokeWidth={1.5} />
        <h1 className="text-lg font-semibold">「{kind.label}」沒有開啟</h1>
        <p className="text-[13px] leading-relaxed text-muted-foreground">
          PathKeep 目前不保存這類資料。以前存下的內容還在存檔裡，開啟後就能再看到。
        </p>
        <div className="flex gap-2">
          <Button variant="outline" onClick={() => void navigate('/settings/kinds')}>
            管理資料類型
          </Button>
          {kind.id === 'agent' && <Button onClick={() => void navigate('/?enable=agent')}>預覽並開啟 {kind.label}</Button>}
        </div>
      </div>
    </div>
  )
}

export function Frame() {
  const { pathname } = useLocation()
  const { enabledKinds, theme, space, set } = useProto()
  const [paletteOpen, setPaletteOpen] = useState(false)
  const kind = kindForPath(pathname) ?? (spaceOfPath(pathname) ? kindById[spaceOfPath(pathname)!] : undefined)
  const off = kind && !enabledKinds.includes(kind.id)
  const pathSpace = spaceOfPath(pathname)
  const leaving = !!pathSpace && pathSpace !== space
  const switchTo = useSwitchSpace()

  // Arriving at a page of another space (a link, ⌘K, back/forward) makes it the current
  // space. Only a change of route counts: the switcher sets the space first and changes
  // the route a moment later, and must not be undone in between.
  useEffect(() => {
    if (pathSpace && enabledKinds.includes(pathSpace)) set({ space: pathSpace })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pathSpace])

  // The tint follows the current space.
  useEffect(() => {
    const html = document.documentElement
    html.dataset.space = space
    html.style.setProperty('--space-h', String(kindById[space].hue))
  }, [space])

  // ⌘1…⌘9 (Ctrl+1…9 on Windows and Linux) switch spaces, in the order of the switcher.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const i = spaceKeyIndex(event)
      if (i === null || i >= enabledKinds.length) return
      event.preventDefault()
      switchTo(enabledKinds[i])
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [enabledKinds, switchTo])

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault()
        setPaletteOpen((open) => !open)
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  return (
    <div className="space-tint relative flex h-full bg-window backdrop-blur-[40px] backdrop-saturate-[1.4]">
      {/* Stand-in for the macOS traffic lights. */}
      <div aria-hidden className="absolute top-[17px] left-[18px] flex gap-2">
        <span className="size-3 rounded-full bg-[#ff5f57]" />
        <span className="size-3 rounded-full bg-[#febc2e]" />
        <span className="size-3 rounded-full bg-[#28c840]" />
      </div>
      <SpaceRail onOpenPalette={() => setPaletteOpen(true)} />
      <main className="mt-2.5 mr-2.5 mb-2.5 flex min-w-0 flex-1 flex-col overflow-hidden rounded-xl border bg-panel">
        {off ? (
          <KindOff kind={kind} />
        ) : (
          <>
            {/* A new space's page fades in after the rail has started to slide. */}
            <motion.div
              key={pathSpace ?? 'shared'}
              initial={{ opacity: 0 }}
              // Leaving: the space already changed but its page hasn't arrived yet.
              animate={{ opacity: leaving ? 0 : 1 }}
              transition={{ duration: leaving ? 0.1 : 0.22, delay: leaving ? 0 : 0.04 }}
              className="flex min-h-0 flex-1"
            >
              <Outlet />
            </motion.div>
          </>
        )}
      </main>
      <Palette open={paletteOpen} onOpenChange={setPaletteOpen} />
      <EnableAgentDialog />
      <Knobs />
      <Toaster theme={theme} position="bottom-center" />
    </div>
  )
}
