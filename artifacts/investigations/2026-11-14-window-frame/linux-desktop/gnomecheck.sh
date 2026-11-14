#!/bin/bash
# gnomecheck.sh MODE SRC_DIR TAG    (MODE: x11 | wayland)
# The same checks as wincheck.sh, under GNOME Shell 46: as the X11 window manager on Xvfb, or as a
# Wayland compositor nested in Xvfb (the app then runs as a Wayland client, GDK_BACKEND=wayland).
# Real pointer and keyboard input go through xdotool on the X display; under Wayland the nested
# compositor passes them on. Window state comes from xprop (X11) or from GNOME Shell's own
# MetaWindow through org.gnome.Shell.Eval (Wayland, shell started in unsafe mode).
set -uo pipefail
MODE=$1; DIR=$2; TAG=$3
OUT=/home/boxd/shots/$TAG
rm -rf "$OUT"; mkdir -p "$OUT"
exec > "$OUT/log.txt" 2>&1
rm -f "/home/boxd/$TAG.CHECK_DONE"
export PATH=$HOME/.cargo/bin:$HOME/.bun/bin:$PATH
export DISPLAY=:99
export XDG_RUNTIME_DIR=/tmp/xdg-boxd; mkdir -p -m 700 $XDG_RUNTIME_DIR
BIN=/home/boxd/pk-target/debug/pathkeep-desktop
BRIDGE=http://127.0.0.1:43117

stop_app() { pkill -f "$BIN"; pkill -f dev-demo.mjs; pkill -f vite; sleep 3; }
stop_app
pkill -f gnome-shell; pkill xfwm4; pkill -f 'Xvfb :99'; sleep 2
Xvfb :99 -screen 0 1920x1200x24 >/dev/null 2>&1 &
sleep 2

if [ "$MODE" = x11 ]; then
  gnome-shell --x11 --replace --unsafe-mode > "$OUT/shell.log" 2>&1 &
else
  export MUTTER_DEBUG_DUMMY_MODE_SPECS=1920x1200
  gnome-shell --nested --wayland --unsafe-mode > "$OUT/shell.log" 2>&1 &
fi
for _ in $(seq 1 60); do gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval '1' >/dev/null 2>&1 && break; sleep 1; done
shelljs() { gdbus call --session --dest org.gnome.Shell --object-path /org/gnome/Shell --method org.gnome.Shell.Eval "$1" 2>&1; }
shelljs 'Main.overview.hide(); "ok"'
echo "shell: $(shelljs 'Meta.is_wayland_compositor()')"
if [ "$MODE" = wayland ]; then
  WAYLAND_DISPLAY=$(ls $XDG_RUNTIME_DIR | grep -E '^wayland-[0-9]+$' | head -1); export WAYLAND_DISPLAY
  export GDK_BACKEND=wayland
  echo "wayland display: $WAYLAND_DISPLAY"
fi

W='global.get_window_actors().map(a => a.meta_window).find(w => w.get_title() === "PathKeep")'
win_js() { shelljs "const w = $W; if (!w) 'none'; else { $1 }"; }
state() { win_js 'const r = w.get_frame_rect(); ({x: r.x, y: r.y, w: r.width, h: r.height, max: w.get_maximized(), min: w.minimized, decorated: w.decorated, client: w.get_client_type()})'; }
geom() { local s; s=$(state); X=$(echo "$s" | grep -o '"x":[-0-9]*' | cut -d: -f2); Y=$(echo "$s" | grep -o '"y":[-0-9]*' | cut -d: -f2); WIDTH=$(echo "$s" | grep -o '"w":[0-9]*' | cut -d: -f2); HEIGHT=$(echo "$s" | grep -o '"h":[0-9]*' | cut -d: -f2); }
record() { echo "RESULT $1: $(state)"; }
shot() { import -window root "$OUT/$1.png"; }
normal_size() { win_js 'w.unminimize(); if (w.get_maximized()) w.unmaximize(Meta.MaximizeFlags.BOTH); w.move_resize_frame(false, 200, 120, 1480, 960); w.activate(global.get_current_time()); "ok"' >/dev/null; sleep 1.5; }
activate() { win_js 'w.unminimize(); w.activate(global.get_current_time()); "ok"' >/dev/null; sleep 1.2; }
click_at() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool mousedown 1; sleep 0.06; xdotool mouseup 1; sleep 1.5; }
tap_at() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool click 1; sleep 0.8; }
dblclick_at() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool mousedown 1; sleep 0.06; xdotool mouseup 1; sleep 0.12; xdotool mousedown 1; sleep 0.06; xdotool mouseup 1; sleep 1.5; }
drag_from() { xdotool mousemove "$1" "$2"; sleep 0.3; xdotool mousedown 1; sleep 0.3; xdotool mousemove_relative -- 60 40; sleep 0.2; xdotool mousemove_relative -- 60 40; sleep 0.3; xdotool mouseup 1; sleep 1.5; }

start_demo() {
  local name=$1; shift
  (cd "$DIR" && nohup bun run dev:demo -- "$@" --backend "$BIN" > "$OUT/demo-$name.log" 2>&1 &)
  for _ in $(seq 1 600); do grep -q 'PathKeep demo is ready' "$OUT/demo-$name.log" && break; sleep 1; done
  for _ in $(seq 1 60); do win_js '"found"' | grep -q found && break; sleep 1; done
  sleep 8; normal_size; activate
}

exercise() {
  local s=$1
  normal_size; record "$s start"; shot "$s"; geom
  click_at $((X + WIDTH - 66)) $((Y + 16)); record "$s after maximize button"; shot "$s-after-maximize"
  geom; click_at $((X + WIDTH - 66)) $((Y + 16)); record "$s after maximize button again"
  normal_size; geom
  click_at $((X + WIDTH - 110)) $((Y + 16)); record "$s after minimize button"
  normal_size; geom
  drag_from $((X + WIDTH - 200)) $((Y + 16)); record "$s after drag from strip (start 200,120; +120,+80 if it drags)"
  normal_size; geom
  dblclick_at $((X + WIDTH - 200)) $((Y + 16)); record "$s after double click on strip"
  normal_size
}
# A press released at once (a trackpad tap) on a drag region, then the pointer moves with no
# button held: does the window follow it?
tap_test() { # label x-offset y-offset
  normal_size; geom
  tap_at $((X + $2)) $((Y + $3))
  xdotool mousemove_relative -- 150 90; sleep 1.2
  record "$1 after a tap and a move with no button held (start 200,120)"
  xdotool key Escape; xdotool click 1; sleep 0.8; normal_size
}

echo "== onboarding"
start_demo onboarding --first-run --fresh
exercise onboarding
geom; drag_from $((X + 130)) $((Y + 12)); record "onboarding after drag from top of step list"
normal_size; geom; drag_from $((X + 130)) $((Y + HEIGHT - 120)); record "onboarding after drag from step list spacer"
tap_test onboarding-margin 600 4
normal_size; geom
click_at $((X + WIDTH - 22)) $((Y + 16)); sleep 4
if pgrep -f "$BIN" >/dev/null; then echo "RESULT onboarding close: app still running"; else echo "RESULT onboarding close: app exited"; fi
stop_app

echo "== set up the archive, then open it"
start_demo init
stop_app
start_demo ready
exercise ready
geom; drag_from $((X + 38)) $((Y + 6)); record "ready after drag from nav rail top"
tap_test navrail-spacer 38 600
tap_test ready-strip $((1480 - 200)) 16
normal_size; geom
drag_from $((X + WIDTH - 2)) $((Y + HEIGHT / 2)); record "ready after drag from right edge (resize?)"
normal_size

echo "== command palette over the main window"
activate; xdotool key ctrl+k; sleep 2; shot ready-palette; geom
click_at $((X + WIDTH - 110)) $((Y + 16)); record "palette after minimize button"
activate; shot ready-palette-after-minimize
xdotool key Escape; sleep 1

echo "== App Lock"
curl -s -X POST -H 'content-type: application/json' "$BRIDGE/commands/set_app_lock_passcode" -d '{"request":{"passcode":"4826"}}' >/dev/null
curl -s -X POST -H 'content-type: application/json' "$BRIDGE/commands/app_snapshot" -d '{}' |
  python3 -c "import json,sys; c=json.load(sys.stdin)['config']; c['appLock']['enabled']=True; print(json.dumps({'config': c}))" > /tmp/pk-config.json
curl -s -X POST -H 'content-type: application/json' "$BRIDGE/commands/save_config" -d @/tmp/pk-config.json >/dev/null
activate; xdotool key ctrl+l; sleep 3
exercise lock
tap_test lock-backdrop 40 400
activate; xdotool type 4826; xdotool key Return; sleep 3; shot ready-after-unlock

echo "== close button"
normal_size; geom
click_at $((X + WIDTH - 22)) $((Y + 16)); sleep 5
if pgrep -f "$BIN" >/dev/null; then echo "RESULT close: app still running"; else echo "RESULT close: app exited"; fi
shot after-close
stop_app
echo DONE; touch "/home/boxd/$TAG.CHECK_DONE"
