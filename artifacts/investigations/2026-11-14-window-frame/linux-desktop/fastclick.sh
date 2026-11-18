#!/bin/bash
# fastclick.sh: does a press released at once (a trackpad tap) on a drag region leave the window
# following the pointer? Clicks, then moves the pointer with no button held, then reads where the
# window is. Compares an instant click with a 60 ms one, on the lock screen's backdrop, the strip
# beside the window buttons, and the nav rail's spacer (a drag region before this change too).
set -uo pipefail
DIR=/home/boxd/pk-fix
OUT=/home/boxd/shots/fastclick
rm -rf "$OUT"; mkdir -p "$OUT"
exec > "$OUT/log.txt" 2>&1
rm -f /home/boxd/fastclick.CHECK_DONE
export PATH=$HOME/.cargo/bin:$HOME/.bun/bin:$PATH
export DISPLAY=:99
BIN=/home/boxd/pk-target/debug/pathkeep-desktop
BRIDGE=http://127.0.0.1:43117

pkill -f "$BIN"; pkill -f dev-demo.mjs; pkill -f vite; sleep 2
pgrep -f 'Xvfb :99' >/dev/null || { Xvfb :99 -screen 0 1920x1200x24 >/dev/null 2>&1 & sleep 2; }
pgrep xfwm4 >/dev/null || { xfwm4 --compositor=off >/dev/null 2>&1 & sleep 3; }

(cd "$DIR" && nohup bun run dev:demo -- --backend "$BIN" > "$OUT/demo.log" 2>&1 &)
WID=
for _ in $(seq 1 240); do WID=$(xdotool search --onlyvisible --name '^PathKeep$' | head -1); [ -n "$WID" ] && break; sleep 0.5; done
for _ in $(seq 1 300); do grep -q 'PathKeep demo is ready' "$OUT/demo.log" && break; sleep 1; done
sleep 8
state() { xprop -id "$WID" _NET_WM_STATE | sed 's/.*= //;s/_NET_WM_STATE_//g'; }
reset() {
  xdotool mousemove 1800 1150; sleep 0.3; xdotool key Escape; sleep 0.3
  wmctrl -i -r "$WID" -b remove,maximized_vert,maximized_horz; sleep 0.5
  xdotool windowsize "$WID" 1480 960; xdotool windowmove "$WID" 100 80; sleep 1.5
  xdotool windowactivate --sync "$WID"; sleep 1
}
where() { eval "$(xdotool getwindowgeometry --shell "$WID")"; echo "x=$X y=$Y"; }
try() { # label x y hold_ms (0 = xdotool click)
  reset; echo "== $1 hold=$4 before: $(where)"
  xdotool mousemove "$2" "$3"; sleep 0.4
  if [ "$4" = 0 ]; then xdotool click 1; else xdotool mousedown 1; sleep "0.0$4"; xdotool mouseup 1; fi
  sleep 0.8
  xdotool mousemove_relative -- 150 90; sleep 1.2
  echo "RESULT $1 hold=$4: after moving the pointer with no button held: $(where) state=[$(state)]"
  import -window root -resize 50% "$OUT/$1-$4.jpg"
  # A click to end a move the window manager might still be in.
  xdotool click 1; sleep 1
}
reset; where
# The app starts locked (App Lock from the earlier run): the lock screen.
import -window root -resize 50% "$OUT/start.jpg"
try lock-backdrop $((X + 40)) $((Y + 400)) 0
try lock-backdrop $((X + 40)) $((Y + 400)) 60
try strip $((X + 1480 - 200)) $((Y + 16)) 0
try strip $((X + 1480 - 200)) $((Y + 16)) 60
# Unlock, then the nav rail's spacer.
reset; xdotool type 4826; xdotool key Return; sleep 3
import -window root -resize 50% "$OUT/unlocked.jpg"
try navrail-spacer $((X + 38)) $((Y + 600)) 0
try navrail-spacer $((X + 38)) $((Y + 600)) 60
pkill -f "$BIN"; pkill -f dev-demo.mjs; pkill -f vite
echo DONE; touch /home/boxd/fastclick.CHECK_DONE
