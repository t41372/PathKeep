import { spawnSync } from 'node:child_process'

// The desktop shell has no host-specific tests of its own any more; what
// check:platform needs from it is that the debug desktop app still builds on
// this host. The native host tests run from run-platform-rust-tests.mjs.
run(['bun', 'run', 'desktop:build:debug'])

function run(command) {
  const [bin, ...args] = command
  const result = spawnSync(bin, args, {
    stdio: 'inherit',
    env: process.env,
  })

  if (result.status !== 0) {
    process.exit(result.status ?? 1)
  }
}
