#!/bin/bash
# usage: remote-bench3.sh <visits> <rounds>   — after the sabotage queue: builds v0.5.0 (with the ported
# benchmark) and runs v050 / base / head alternating, <rounds> times each, test archive on disk.
# Then v050 once at 14.4M under an 8 GiB limit (3 h cap). Results: bench3-<visits>.jsonl/.log,
# bench3-big-v050.jsonl, marker bench3.DONE.
set -uo pipefail
N=$1; R=$2
cd /home/boxd
export PATH=$HOME/.cargo/bin:$PATH TMPDIR=/home/boxd/benchtmp
while [ ! -f sabotage-cand.DONE ]; do sleep 20; done
out=/home/boxd/bench3-$N.jsonl; log=/home/boxd/bench3-$N.log; rm -f bench3.DONE; : > $out
{
  code=0
  echo "== build v050 $(date -u +%T)"
  (cd v050/src-tauri && CARGO_TARGET_DIR=/home/boxd/target-v050 cargo test -p vault-core --release --lib --no-run 2>&1 | tail -2) || code=1
  if [ $code -eq 0 ]; then
    for round in $(seq 1 $R); do
      for side in v050 base head; do
        echo "== run $side round $round N=$N $(date -u +%T)"
        (cd $side/src-tauri && CARGO_TARGET_DIR=/home/boxd/target-$side PATHKEEP_SEMANTIC_VISITS=$N PATHKEEP_SEMANTIC_APPEND_VISITS=64 \
          /usr/bin/time -v cargo test -p vault-core --release --lib semantic_index_update_benchmark -- --ignored --nocapture --test-threads=1) > run3.tmp 2>&1
        rc=$?; grep '^{' run3.tmp | sed "s/^{/{\"side\":\"$side\",\"round\":$round,/" >> $out
        grep -E 'Maximum resident|Elapsed \(wall|test result|panicked|^Error' run3.tmp | sed "s/^/$side r$round: /"
        [ $rc -eq 0 ] || code=$rc
      done
    done
    echo "== v050 N=14400000 under 8G $(date -u +%T)"
    bin=$(cd v050/src-tauri && CARGO_TARGET_DIR=/home/boxd/target-v050 cargo test -p vault-core --release --lib --no-run --message-format=json 2>/dev/null | python3 -c "
import sys, json
for line in sys.stdin:
    try: d = json.loads(line)
    except Exception: continue
    if d.get('reason') == 'compiler-artifact' and d.get('executable') and d['target']['name'] == 'vault_core': print(d['executable'])
" | tail -1)
    sudo systemd-run --scope --quiet -p MemoryMax=8G -p MemorySwapMax=0 --uid=boxd --setenv=PATHKEEP_SEMANTIC_VISITS=14400000 --setenv=PATHKEEP_SEMANTIC_APPEND_VISITS=64 --setenv=HOME=$HOME --setenv=TMPDIR=$TMPDIR \
      timeout 10800 /usr/bin/time -v "$bin" semantic_index_update_benchmark --ignored --nocapture --test-threads=1 > /home/boxd/run3-big.tmp 2>&1
    echo "big v050 exit $?"; grep '^{' /home/boxd/run3-big.tmp > /home/boxd/bench3-big-v050.jsonl
    grep -E 'Maximum resident|Elapsed \(wall|test result|panicked|signal' /home/boxd/run3-big.tmp
    rm -rf /home/boxd/benchtmp/.tmp* 2>/dev/null
  fi
  echo "BENCH3 EXIT $code $(date -u +%T)"; df -h / | tail -1
} > $log 2>&1
echo $code > bench3.DONE
