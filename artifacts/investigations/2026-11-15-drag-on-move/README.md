# A tap on a drag region on Xfce (WORK-DRAG-ON-MOVE), 2026-11-15

On xfwm4, Xfce's window manager, a press released at once on a drag region (a trackpad tap)
left the window following the pointer until the next click. Tauri 2.12.5's `drag.js` asks the
window manager to move the window on the press; after a tap the request arrives once the button
is already up. On Windows and Linux the marks are now PathKeep's own (`data-window-drag`), and
`src/app/window-chrome/drag.ts` asks for the move only once the pointer has moved 4 px with the
button down.

All runs on one boxd machine (Ubuntu 24.04, x86_64, 4 vCPU), the way CI's gate runs the
real-window spec (`CI=1`, `PATHKEEP_E2E_NATIVE_WINDOW=required`, `dbus-run-session`,
`xvfb-run`): `first-run-native-window.spec.ts` checks the buttons, a drag and a double click
under mutter, then the same under xfwm4, then taps three spots on the top strip twice each and
moves the pointer with no button down.

- `red.txt`: the branch with the drag code from before the fix put back (`run.sh red`). Every
  check under mutter and xfwm4 passed; the first tap at each of the three spots left the window
  following the pointer, from (40, 40) to (200, 130).
- `green.txt`: the branch as it is (`run.sh green`): passed, 36.8 s.
- `stress.txt`: the fixed tree ten times in a row with all four cores kept busy by stress-ng
  (`stress.sh`): 10 of 10 passed, 28.7–29.5 s each. A first try of this found a fault in the
  spec itself, not the app: from the second run on, xfwm4 from the run before was still on the
  display, and the spec, which then only swapped window managers it had started itself, refused
  to swap and failed nine times. It now takes CI's `required` to mean the display is its own and
  puts mutter, then xfwm4, on it whatever ran before.
- `wmcheck.sh`: swapping mutter and xfwm4 on one Xvfb display, both ways, and the names
  `wmctrl -m` reports for them (`Mutter`, `Xfwm4`).
- `setup.sh`: the machine's setup (CI's packages plus xfwm4).

Not checked: Windows, KDE, and the macOS window (which keeps Tauri's marks).
