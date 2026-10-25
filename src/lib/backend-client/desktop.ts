/**
 * Desktop integration the OS owns: opening at login and the menu bar /
 * system tray icon. Each setter returns the new state as the OS sees it.
 * Also the question the app asks when someone quits while work runs.
 */
import { call } from './shared'

export interface DesktopIntegration {
  launchAtLogin: boolean
  menuBarIcon: boolean
  /** False where this OS or build can't do it; hide the row then. */
  launchAtLoginSupported: boolean
  menuBarIconSupported: boolean
}

export const desktopClient = {
  get: () => call<DesktopIntegration>('get_desktop_integration'),
  setLaunchAtLogin: (enabled: boolean) =>
    call<DesktopIntegration>('set_launch_at_login', { enabled }),
  setMenuBarIcon: (enabled: boolean) =>
    call<DesktopIntegration>('set_menu_bar_icon', { enabled }),
}

/**
 * Work someone started and is waiting on (`vault_worker::ForegroundWork`):
 * quitting while it runs asks first.
 */
export type ForegroundWork =
  | 'backup'
  | 'history-export'
  | 'history-import'
  | 'move-export'
  | 'move-import'
  | 'restore'
  | 'free-up-space'
  | 'archive-upgrade'
  | 'password-change'
  | 'encryption-change'
  | 'data-wipe'
  | 'import-undo'

/** A quit the backend holds while work runs, waiting for an answer. */
export interface QuitRequest {
  /** Answers name the question they answer; an older one's are ignored. */
  id: number
  /** What is running, each kind once, oldest first; empty once it all ended. */
  work: ForegroundWork[]
  /** They chose to wait: PathKeep quits as soon as `work` is empty. */
  waiting: boolean
}

export type QuitAnswer = 'wait' | 'quit-now' | 'keep-open'

export const quitClient = {
  /** The open question, or `null`; for a window that missed its event. */
  get: () => call<QuitRequest | null>('get_quit_request'),
  /** Returns the question as it stands after the answer, `null` once closed. */
  answer: (id: number, answer: QuitAnswer) =>
    call<QuitRequest | null>('answer_quit_request', { id, answer }),
}
