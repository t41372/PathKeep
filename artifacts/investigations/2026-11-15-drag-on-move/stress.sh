#!/bin/bash
# stress.sh: the real-window spec (fixed tree) 10 times in a row, the way CI runs the gate, with all
# four cores kept busy by stress-ng.
exec > /home/boxd/stress.log 2>&1
rm -f /home/boxd/stress.DONE
export PATH=$HOME/.cargo/bin:$HOME/.bun/bin:$PATH CI=1 PATHKEEP_E2E_NATIVE_WINDOW=required
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y stress-ng >/dev/null
pkill -x mutter; pkill -x xfwm4; pkill -x Xvfb; sleep 2
tar -xzf /home/boxd/tests-update.tgz -C /home/boxd/pk
cd /home/boxd/pk
nproc
stress-ng --cpu 4 --timeout 3600s --quiet &
STRESS=$!
dbus-run-session -- xvfb-run -a node scripts/run-playwright.mjs --project=first-run --repeat-each 10 --retries 0 first-run-native-window
echo "exit=$?"
kill $STRESS
touch /home/boxd/stress.DONE
