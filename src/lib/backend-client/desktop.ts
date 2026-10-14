/**
 * Desktop integration the OS owns: opening at login and the menu bar /
 * system tray icon. Each setter returns the new state as the OS sees it.
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
