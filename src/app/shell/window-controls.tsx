/**
 * Minimize, maximize (restore) and close for Windows and Linux, drawn by the
 * app at the top right of the window background: the system title bar is off
 * there, so the window is the same two layers as on macOS, with these where
 * the traffic lights sit on the left (`back-forward-window-frame-tradeoff.md` D).
 *
 * Not responsible for snap layouts on Windows 11 (the backend answers the
 * system's hit test over the maximize button, `window_frame.rs`) or dragging
 * (the strip around it is a drag region).
 */
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useEffect, useState } from 'react'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'

const button =
  'flex h-8 w-11 items-center justify-center text-foreground/80 transition-colors hover:bg-foreground/10 focus-visible:bg-foreground/10 focus-visible:outline-none'

export function WindowControls() {
  const { t } = useI18n()
  const [maximized, setMaximized] = useState(false)
  useEffect(() => {
    const appWindow = getCurrentWindow()
    let stop: (() => void) | undefined
    let live = true
    const read = () =>
      void appWindow.isMaximized().then((value) => live && setMaximized(value))
    read()
    void appWindow.onResized(read).then((unlisten) => {
      if (live) stop = unlisten
      else unlisten()
    })
    return () => {
      live = false
      stop?.()
    }
  }, [])

  const appWindow = () => getCurrentWindow()
  return (
    <div
      className="absolute top-0 right-0 z-50 flex"
      role="group"
      aria-label={t('shell.window.label')}
    >
      <button
        type="button"
        aria-label={t('shell.window.minimize')}
        title={t('shell.window.minimize')}
        className={button}
        onClick={() => void appWindow().minimize()}
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
        onClick={() => void appWindow().toggleMaximize()}
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
        onClick={() => void appWindow().close()}
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
