#!/bin/bash
# build.sh SRC_TARBALL DIR: unpack a PathKeep source tree and build the debug desktop app with the dev bridge.
set -euo pipefail
TARBALL=$1; DIR=$2
LOG=/home/boxd/build-$(basename $DIR).log
exec > $LOG 2>&1
rm -f $DIR.BUILD_DONE $DIR.BUILD_FAILED
trap 'echo FAILED; touch $DIR.BUILD_FAILED' ERR
export PATH=$HOME/.cargo/bin:$HOME/.bun/bin:$PATH
export CARGO_TARGET_DIR=/home/boxd/pk-target
rm -rf $DIR && mkdir -p $DIR && tar -xzf $TARBALL -C $DIR
cd $DIR
bun install --frozen-lockfile
bun run prepare:worker-sidecar --profile debug
cargo build --manifest-path src-tauri/Cargo.toml --features devtools-bridge
ls -la $CARGO_TARGET_DIR/debug/pathkeep-desktop
echo OK; touch $DIR.BUILD_DONE
