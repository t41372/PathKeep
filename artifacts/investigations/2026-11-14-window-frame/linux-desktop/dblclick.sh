#!/bin/bash
# dblclick.sh: why a double click on a drag region does not maximize on Linux.
# Logs every mousedown's detail in the real window (console.warn reaches Vite's output), then
# double-clicks drag regions with xdotool at several intervals and records the window state.
set -uo pipefail
DIR=/home/boxd/pk-fix
OUT=/home/boxd/shots/dblclick
rm -rf "$OUT"; mkdir -p "$OUT"
exec > "$OUT/log.txt" 2>&1
rm -f /home/boxd/dblclick.CHECK_DONE
export PATH=$HOME/.cargo/bin:$HOME/.bun/bin:$PATH
export DISPLAY=:99
BIN=/home/boxd/pk-target/debug/pathkeep-desktop

pkill -f "$BIN"; pkill -f dev-demo.mjs; pkill -f vite; sleep 2
pgrep -f 'Xvfb :99' >/dev/null || { Xvfb :99 -screen 0 1920x1200x24 >/dev/null 2>&1 & sleep 2; }
pgrep xfwm4 >/dev/null || { xfwm4 --compositor=off >/dev/null 2>&1 & sleep 3; }

# Debug only, in this copy: log what the page sees on mousedown and dblclick.
cp "$DIR/src/main.tsx" /tmp/main.tsx.orig
cat >> "$DIR/src/main.tsx" <<'JS'

for (const type of ['mousedown', 'mouseup', 'dblclick'])
  document.addEventListener(type, (e) => console.warn(`PKDBG ${type} detail=${e.detail} target=${(e.target as Element).tagName} drag=${(e.target as Element).getAttribute?.('data-tauri-drag-region')}`), true)
JS

(cd "$DIR" && nohup bun run dev:demo -- --backend "$BIN" > "$OUT/demo.log" 2>&1 &)
WID=
for _ in $(seq 1 240); do WID=$(xdotool search --onlyvisible --name '^PathKeep$' | head -1); [ -n "$WID" ] && break; sleep 0.5; done
for _ in $(seq 1 300); do grep -q 'PathKeep demo is ready' "$OUT/demo.log" && break; sleep 1; done
sleep 8
xdotool windowactivate --sync "$WID"; sleep 1
state() { xprop -id "$WID" _NET_WM_STATE | sed 's/.*= //;s/_NET_WM_STATE_//g'; }
reset() { wmctrl -i -r "$WID" -b remove,maximized_vert,maximized_horz; sleep 0.5; xdotool windowsize "$WID" 1480 960; xdotool windowmove "$WID" 100 80; sleep 1.5; }
mark() { echo "MARK $(date -u +%H:%M:%S) $1"; sleep 0.5; }
reset
eval "$(xdotool getwindowgeometry --shell "$WID")"
try() { # label x y delay_ms
  reset; mark "$1 delay=$4"
  xdotool mousemove "$2" "$3"; sleep 0.4
  xdotool click --repeat 2 --delay "$4" 1; sleep 2
  echo "RESULT $1 delay=$4: state=[$(state)] geom=$(xdotool getwindowgeometry "$WID" | tr '\n' ' ')"
}
for d in 60 150 300; do
  try strip $((X + WIDTH - 200)) $((Y + 16)) $d
  try navrail-spacer $((X + 38)) $((Y + 600)) $d
done
# Press, release, press again by hand, slowly enough for the window manager to finish the move.
reset; mark manual
xdotool mousemove $((X + WIDTH - 200)) $((Y + 16)); sleep 0.4
xdotool mousedown 1; sleep 0.05; xdotool mouseup 1; sleep 0.12; xdotool mousedown 1; sleep 0.05; xdotool mouseup 1; sleep 2
echo "RESULT manual: state=[$(state)]"
sleep 2
grep PKDBG "$OUT/demo.log" > "$OUT/events.txt"
pkill -f "$BIN"; pkill -f dev-demo.mjs; pkill -f vite
cp /tmp/main.tsx.orig "$DIR/src/main.tsx"
echo DONE; touch /home/boxd/dblclick.CHECK_DONE
