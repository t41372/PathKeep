#!/bin/bash
# run.sh red|green: the real-window spec the way CI's gate runs it (CI=1, the native window
# required, a session bus, xvfb-run). red puts back the drag code from before the fix (Tauri's
# mark started the drag on the press); green is the branch as it is.
set -uo pipefail
MODE=$1
OUT=/home/boxd/out-$MODE
rm -rf $OUT; mkdir -p $OUT
exec > $OUT/log.txt 2>&1
rm -f /home/boxd/run-$MODE.DONE
export PATH=$HOME/.cargo/bin:$HOME/.bun/bin:$PATH CI=1 PATHKEEP_E2E_NATIVE_WINDOW=required
pkill -f mutter; pkill xfwm4; pkill -f Xvfb; sleep 2
cd /home/boxd/pk
tar -xzf /home/boxd/tree.tgz src tests
if [ "$MODE" = red ]; then
  tar -xzf /home/boxd/red-app.tgz
  rm -f src/app/window-chrome/drag.ts
fi
grep -c "data-window-drag" src/components/app/drag-region.ts || true
dbus-run-session -- xvfb-run -a node scripts/run-playwright.mjs --project=first-run first-run-native-window
echo "exit=$?"
cp -r artifacts/e2e/report artifacts/e2e/results.json artifacts/e2e/test-results $OUT/ 2>/dev/null
touch /home/boxd/run-$MODE.DONE
