#!/bin/bash
# wincheck.sh SRC_DIR TAG
# Runs the real PathKeep desktop window (debug build, synthetic demo data) on an X display with a
# window manager, and drives it with real pointer and keyboard input: onboarding, the main window,
# the command palette over it, App Lock, then close. Records the window's geometry and
# _NET_WM_STATE after each action, and screenshots, under /home/boxd/shots/TAG.
set -uo pipefail
DIR=$1; TAG=$2
OUT=/home/boxd/shots/$TAG
rm -rf "$OUT"; mkdir -p "$OUT"
exec > "$OUT/log.txt" 2>&1
rm -f "/home/boxd/$TAG.CHECK_DONE"
export PATH=$HOME/.cargo/bin:$HOME/.bun/bin:$PATH
export DISPLAY=:99
BIN=/home/boxd/pk-target/debug/pathkeep-desktop
BRIDGE=http://127.0.0.1:43117

cleanup_app() {
  pkill -f "$BIN" 2>/dev/null; pkill -f 'scripts/dev-demo.mjs' 2>/dev/null; pkill -f vite 2>/dev/null
  sleep 3
}
cleanup_app
pkill -f 'Xvfb :99' 2>/dev/null; pkill xfwm4 2>/dev/null; sleep 1
Xvfb :99 -screen 0 1920x1200x24 >/dev/null 2>&1 &
sleep 2
xfwm4 --compositor=off >/dev/null 2>&1 &
sleep 3

WID=
shot() { import -window root "$OUT/$1.png"; echo "shot $1"; }
state() { xprop -id "$WID" _NET_WM_STATE 2>/dev/null | sed 's/.*= //;s/_NET_WM_STATE_//g'; }
geom() { eval "$(xdotool getwindowgeometry --shell "$WID")"; }
record() { geom; echo "RESULT $1: x=$X y=$Y w=$WIDTH h=$HEIGHT state=[$(state)]"; }
click_at() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 1.5; }
# A person holds each press for a moment; xdotool's own double click releases at once, before
# the drag Tauri starts on the first press reaches the window manager, which then eats the second.
dblclick_at() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool mousedown 1; sleep 0.06; xdotool mouseup 1; sleep 0.12; xdotool mousedown 1; sleep 0.06; xdotool mouseup 1; sleep 1.5; }
drag_from() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool mousedown 1; sleep 0.3; xdotool mousemove_relative -- 60 40; sleep 0.2; xdotool mousemove_relative -- 60 40; sleep 0.3; xdotool mouseup 1; sleep 1.5; }
activate() { xdotool windowmap "$WID" 2>/dev/null; xdotool windowactivate --sync "$WID" 2>/dev/null; sleep 1.5; }
normal_size() {
  # Put the window back where every run starts: unmaximized, 1480x960 at 100,80.
  wmctrl -i -r "$WID" -b remove,maximized_vert,maximized_horz 2>/dev/null; sleep 0.5
  xdotool windowsize "$WID" 1480 960; xdotool windowmove "$WID" 100 80; sleep 1.5
}

start_demo() { # args for dev:demo; waits for the window and the ready line
  local name=$1; shift
  (cd "$DIR" && nohup bun run dev:demo -- "$@" --backend "$BIN" > "$OUT/demo-$name.log" 2>&1 &)
  WID=
  for _ in $(seq 1 240); do
    WID=$(xdotool search --onlyvisible --name '^PathKeep$' 2>/dev/null | head -1)
    [ -n "$WID" ] && break
    sleep 0.5
  done
  echo "window $name: $WID"
  # The first seconds after the window shows: before and while the page loads.
  for i in $(seq 1 10); do import -window root -resize 50% "$OUT/$name-startup-$i.jpg"; sleep 0.3; done
  for _ in $(seq 1 600); do grep -q 'PathKeep demo is ready' "$OUT/demo-$name.log" && break; sleep 1; done
  sleep 8
  activate
}

# Every surface gets the same actions at the same places: the three buttons' centers (44x32 each
# at the top right), the strip left of them, a double click on that strip.
exercise() { # surface
  local s=$1
  normal_size; activate; record "$s start"; shot "$s"
  geom
  click_at $((X + WIDTH - 66)) $((Y + 16)); record "$s after maximize button"; shot "$s-after-maximize"
  click_at $((X + WIDTH - 66)) $((Y + 16)); record "$s after maximize button again"
  normal_size; activate; geom
  click_at $((X + WIDTH - 110)) $((Y + 16)); record "$s after minimize button"
  activate; normal_size; activate; geom
  drag_from $((X + WIDTH - 200)) $((Y + 16)); record "$s after drag from strip (expect +120,+80 if it drags)"
  normal_size; activate; geom
  dblclick_at $((X + WIDTH - 200)) $((Y + 16)); record "$s after double click on strip"
  normal_size; activate
}

echo "== onboarding"
start_demo onboarding --first-run --fresh
exercise onboarding
geom
drag_from $((X + 130)) $((Y + 12)); record "onboarding after drag from top of step list"
normal_size; activate; geom
drag_from $((X + 130)) $((Y + HEIGHT - 120)); record "onboarding after drag from step list spacer"
normal_size; activate; geom
click_at $((X + WIDTH - 22)) $((Y + 16)); sleep 4
if pgrep -f "$BIN" >/dev/null; then echo "RESULT onboarding close: app still running"; else echo "RESULT onboarding close: app exited"; fi
cleanup_app

echo "== set up the archive (dev:demo initializes it and backs up), then open it"
start_demo init
cleanup_app
start_demo ready
exercise ready
geom
drag_from $((X + 38)) $((Y + 6)); record "ready after drag from nav rail top"
normal_size; activate; geom
dblclick_at $((X + WIDTH - 200)) $((Y + 16)); record "ready after a second double click on strip"; shot ready-after-dblclick
dblclick_at $((X + WIDTH - 200)) $((Y + 16)); record "ready after double click on strip again"
normal_size; activate
geom
drag_from $((X + WIDTH - 2)) $((Y + HEIGHT / 2)); record "ready after drag from right edge (resize?)"
normal_size; activate

echo "== command palette over the main window"
xdotool key ctrl+k; sleep 2; shot ready-palette; geom
click_at $((X + WIDTH - 110)) $((Y + 16)); record "palette after minimize button"
activate; shot ready-palette-after-minimize
xdotool key Escape; sleep 1

echo "== App Lock"
curl -s -X POST -H 'content-type: application/json' "$BRIDGE/commands/set_app_lock_passcode" \
  -d '{"request":{"passcode":"4826"}}' | head -c 300; echo
# Turn App Lock on, as its switch in Settings does.
curl -s -X POST -H 'content-type: application/json' "$BRIDGE/commands/app_snapshot" -d '{}' |
  python3 -c "import json,sys; c=json.load(sys.stdin)['config']; c['appLock']['enabled']=True; print(json.dumps({'config': c}))" > /tmp/pk-config.json
curl -s -X POST -H 'content-type: application/json' "$BRIDGE/commands/save_config" -d @/tmp/pk-config.json | head -c 200; echo
activate; xdotool key ctrl+l; sleep 3
exercise lock
xdotool type 4826; xdotool key Return; sleep 3; shot ready-after-unlock

echo "== close button"
normal_size; activate; geom
click_at $((X + WIDTH - 22)) $((Y + 16)); sleep 5
if pgrep -f "$BIN" >/dev/null; then
  echo "RESULT close: app still running, window visible=$(xdotool search --onlyvisible --name '^PathKeep$' | head -1)"
else
  echo "RESULT close: app exited"
fi
shot after-close
cleanup_app
echo DONE; touch "/home/boxd/$TAG.CHECK_DONE"
