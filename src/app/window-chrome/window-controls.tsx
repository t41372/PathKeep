/**
 * Minimize, maximize (restore) and close on Windows and Linux, where the
 * window has no system title bar (`windowFrame` is `drawn`): drawn by the app
 * at the top right, where the system's buttons would be, the way the traffic
 * lights sit at the top left on macOS (`back-forward-window-frame-tradeoff.md`
 * D).
 *
 * Mounted once at the app root (`src/app/index.tsx`), beside whatever the
 * session shows, because the window is the same window on every screen:
 * onboarding, the lock screens, starting, the upgrade screens and the error
 * views have no title bar either. They stand in for the system's buttons, so
 * they also stay usable while a dialog is open: above every overlay, taking
 * clicks although a modal dialog turns pointer events off for the rest of the
 * page, and without the dialog taking the click for one outside it or losing
 * the focus.
 *
 * Responsible for: the buttons and the maximized state they show. Not
 * responsible for: the room each screen leaves for them (`frame.ts`),
 * dragging (the window background around them is a drag region,
 * `drag-region.ts`), or Windows 11's snap layouts, which are not done
 * (BACKLOG `WORK-WINDOWS-SNAP-LAYOUTS`; `data-window-maximize` marks the
 * button for them).
 */
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useEffect, useState } from 'react'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { drawsWindowControls, hasTauriGuestApi } from '@/lib/runtime'

const button =
  'flex h-8 w-11 items-center justify-center text-foreground/80 transition-colors hover:bg-foreground/10 focus-visible:bg-foreground/10 focus-visible:outline-none'

/** The window, or none when the dev bridge's page stands in for it (`windowFrame`). */
const appWindow = () => (hasTauriGuestApi() ? getCurrentWindow() : null)

export function WindowControls() {
  if (!drawsWindowControls()) return null
  return <Buttons />
}

function Buttons() {
  const { t } = useI18n()
  const [maximized, setMaximized] = useState(false)
  useEffect(() => {
    const current = appWindow()
    if (!current) return
    let stop: (() => void) | undefined
    let live = true
    const read = () =>
      void current.isMaximized().then((value) => live && setMaximized(value))
    read()
    void current.onResized(read).then((unlisten) => {
      if (live) stop = unlisten
      else unlisten()
    })
    return () => {
      live = false
      stop?.()
    }
  }, [])

  return (
    <div
      className="pointer-events-auto fixed top-0 right-0 z-[60] flex"
      role="group"
      aria-label={t('shell.window.label')}
      // An open dialog would take this press for one outside it and close.
      onPointerDown={(event) => event.stopPropagation()}
      // Like the system's title bar, the buttons leave the focus where it is.
      onMouseDown={(event) => event.preventDefault()}
    >
      <button
        type="button"
        aria-label={t('shell.window.minimize')}
        title={t('shell.window.minimize')}
        className={button}
        onClick={() => void appWindow()?.minimize()}
      >
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden>
          <path d="M0 5h10" stroke="currentColor" strokeWidth="1" />
        </svg>
      </button>
      <button
        type="button"
        data-window-maximize
        aria-label={t(
          maximized ? 'shell.window.restore' : 'shell.window.maximize',
        )}
        title={t(maximized ? 'shell.window.restore' : 'shell.window.maximize')}
        className={button}
        onClick={() => void appWindow()?.toggleMaximize()}
      >
        {maximized ? (
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden>
            <path
              d="M2.5 2.5V.5h7v7h-2M.5 2.5h7v7h-7z"
              fill="none"
              stroke="currentColor"
              strokeWidth="1"
            />
          </svg>
        ) : (
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden>
            <rect
              x=".5"
              y=".5"
              width="9"
              height="9"
              fill="none"
              stroke="currentColor"
              strokeWidth="1"
            />
          </svg>
        )}
      </button>
      <button
        type="button"
        aria-label={t('shell.window.close')}
        title={t('shell.window.close')}
        className={cn(
          button,
          'hover:bg-red hover:text-white focus-visible:bg-red focus-visible:text-white',
        )}
        onClick={() => void appWindow()?.close()}
      >
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden>
          <path
            d="M0 0l10 10M10 0L0 10"
            stroke="currentColor"
            strokeWidth="1"
          />
        </svg>
      </button>
    </div>
  )
}
