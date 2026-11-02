/**
 * The question PathKeep asks when someone quits while work they are waiting
 * on runs (closing the window with the menu bar icon off, ⌘Q, the icon's
 * Quit): what is running, what quitting now does to each, and three answers:
 * quit when it is done, quit now, or keep PathKeep open.
 *
 * Responsible for: showing the question the backend holds and sending the
 * answer. Not responsible for: holding the quit, knowing what runs, or
 * quitting (`src-tauri/src/desktop_integration/quit.rs`).
 *
 * Mounted at the app root, so it shows over every screen, the lock screen
 * and onboarding included; it names kinds of work only, nothing from the
 * archive. Escape keeps PathKeep open. When it opens the focus goes to the
 * dialog itself, not to a button, so a stray Enter answers nothing and Tab
 * reaches the buttons.
 */
import { useEffect, useRef, useState } from 'react'
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { Spinner } from '@/components/ui/spinner'
import {
  quitClient,
  type ForegroundWork,
  type QuitAnswer,
  type QuitRequest,
} from '@/lib/backend-client/desktop'
import { useI18n, type MessageKey } from '@/lib/i18n'
import { subscribeToQuitRequest } from '@/lib/ipc/desktop-events'

/** What quitting now does to each kind of work. */
const IF_QUIT: Record<ForegroundWork, MessageKey> = {
  backup: 'shell.quit.ifQuit.backup',
  'history-export': 'shell.quit.ifQuit.export',
  'move-export': 'shell.quit.ifQuit.export',
  'history-import': 'shell.quit.ifQuit.import',
  'move-import': 'shell.quit.ifQuit.archive',
  restore: 'shell.quit.ifQuit.archive',
  'free-up-space': 'shell.quit.ifQuit.archive',
  'archive-upgrade': 'shell.quit.ifQuit.archive',
  'password-change': 'shell.quit.ifQuit.archive',
  'encryption-change': 'shell.quit.ifQuit.archive',
  'data-wipe': 'shell.quit.ifQuit.wipe',
  'import-undo': 'shell.quit.ifQuit.undo',
}

export function QuitPrompt() {
  const { t } = useI18n()
  const [request, setRequest] = useState<QuitRequest | null>(null)
  const [answering, setAnswering] = useState(false)
  // Events arrive in the order the backend changed the question, so a reply
  // that comes back after an event may be older than it: replies are kept
  // only while no event has arrived since their request went out.
  const events = useRef(0)

  useEffect(() => {
    let live = true
    const unsubscribe = subscribeToQuitRequest((next) => {
      events.current += 1
      if (live) setRequest(next)
    })
    // A window that opens or reloads after the question was sent reads it,
    // once listening: read first, and a question sent in between was missed.
    void unsubscribe
      .then(() => {
        if (!live) return
        const before = events.current
        return quitClient.get().then((current) => {
          if (live && events.current === before) setRequest(current)
        })
      })
      .catch(() => {})
    return () => {
      live = false
      void unsubscribe.then((stop) => stop())
    }
  }, [])

  const answer = (choice: QuitAnswer) => {
    if (!request || answering) return
    setAnswering(true)
    const before = events.current
    quitClient
      .answer(request.id, choice)
      .then((next) => {
        if (events.current === before) setRequest(next)
      })
      // Quitting now ends the app before it can reply; anything else leaves
      // the question to the next event.
      .catch(() => {})
      .finally(() => setAnswering(false))
  }

  const done = request !== null && request.work.length === 0
  const title = done
    ? t('shell.quit.doneTitle')
    : request?.waiting
      ? t('shell.quit.waitingTitle')
      : t('shell.quit.title')
  const body = done
    ? t('shell.quit.doneBody')
    : request?.waiting
      ? t('shell.quit.waitingBody')
      : t('shell.quit.body')

  return (
    <AlertDialog
      open={request !== null}
      onOpenChange={(open) => {
        if (!open) answer('keep-open')
      }}
    >
      <AlertDialogContent
        onOpenAutoFocus={(event) => {
          // Radix would focus a cancel button; this dialog has none.
          event.preventDefault()
          const dialog = event.currentTarget as HTMLElement
          dialog.focus({ preventScroll: true })
        }}
      >
        <AlertDialogHeader>
          <AlertDialogTitle className="flex items-center gap-2">
            {/* The title says it; the spinner's own "Loading" stays out of the name. */}
            {request?.waiting && !done && (
              <Spinner aria-hidden className="size-4" />
            )}
            {title}
          </AlertDialogTitle>
          <AlertDialogDescription>{body}</AlertDialogDescription>
        </AlertDialogHeader>
        {request && !done && (
          <ul
            aria-label={t('shell.quit.running')}
            className="flex flex-col gap-2 text-sm"
          >
            {request.work.map((work) => (
              <li key={work} className="flex flex-col gap-0.5">
                <span className="font-medium">
                  {t(`shell.quit.work.${work}`)}
                </span>
                <span className="text-muted-foreground">
                  {t(IF_QUIT[work])}
                </span>
              </li>
            ))}
          </ul>
        )}
        <AlertDialogFooter>
          <Button
            variant="outline"
            disabled={answering}
            onClick={() => answer('keep-open')}
          >
            {t('shell.quit.keepOpen')}
          </Button>
          {done ? (
            <Button disabled={answering} onClick={() => answer('quit-now')}>
              {t('shell.quit.quit')}
            </Button>
          ) : (
            <>
              <Button
                variant="outline"
                disabled={answering}
                onClick={() => answer('quit-now')}
              >
                {t('shell.quit.quitNow')}
              </Button>
              {!request?.waiting && (
                <Button disabled={answering} onClick={() => answer('wait')}>
                  {t('shell.quit.wait')}
                </Button>
              )}
            </>
          )}
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
