#!/bin/bash
# The real-window spec 20 times in a row, the way CI runs the gate, with all four cores kept busy.
exec > /home/boxd/stress-run.log 2>&1
rm -f /home/boxd/stress-run.CHECK_DONE
export PATH=$HOME/.cargo/bin:$HOME/.bun/bin:$PATH CI=1 PATHKEEP_E2E_NATIVE_WINDOW=required
cd /home/boxd/pk
nproc
stress-ng --cpu 4 --timeout 3600s --quiet &
STRESS=$!
dbus-run-session -- xvfb-run -a node scripts/run-playwright.mjs --project first-run --repeat-each 20 first-run-native-window
echo "exit=$?"
kill $STRESS
touch /home/boxd/stress-run.CHECK_DONE
