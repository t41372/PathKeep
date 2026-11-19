#!/bin/bash
# usage: remote-bench-big.sh <visits>  — waits for the alternating run to finish, then runs the head
# build once at <visits> under an 8 GiB memory limit with no swap (the target machine's RAM).
# Results: bench-big-<visits>.jsonl/.log, marker bench-big-<visits>.DONE with the exit code.
set -uo pipefail
N=$1
cd /home/boxd
export PATH=$HOME/.cargo/bin:$PATH
while [ ! -f bench-1000000.DONE ]; do sleep 20; done
out=bench-big-$N.jsonl; log=bench-big-$N.log; rm -f bench-big-$N.DONE; : > $out
{
  echo "== head N=$N under MemoryMax=8G $(date -u +%T)"; free -g | head -2
  cd head/src-tauri
  bin=$(CARGO_TARGET_DIR=/home/boxd/target-head cargo test -p vault-core --release --lib --no-run --message-format=json 2>/dev/null | python3 -c "
import sys, json
for line in sys.stdin:
    try: d = json.loads(line)
    except Exception: continue
    if d.get('reason') == 'compiler-artifact' and d.get('executable') and d['target']['name'] == 'vault_core': print(d['executable'])
" | tail -1)
  echo "test binary: $bin"
  sudo systemd-run --scope --quiet -p MemoryMax=8G -p MemorySwapMax=0 --uid=boxd --setenv=PATHKEEP_SEMANTIC_VISITS=$N --setenv=PATHKEEP_SEMANTIC_APPEND_VISITS=64 --setenv=HOME=$HOME \
    /usr/bin/time -v "$bin" semantic_index_update_benchmark --ignored --nocapture --test-threads=1 > /home/boxd/run-big.tmp 2>&1
  code=$?
  tail -40 /home/boxd/run-big.tmp
  grep '^{' /home/boxd/run-big.tmp >> /home/boxd/$out
  echo "BENCH EXIT $code $(date -u +%T)"; df -h / | tail -1
} > $log 2>&1
echo $code > bench-big-$N.DONE
