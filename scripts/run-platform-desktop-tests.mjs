import { spawnSync } from 'node:child_process'

run(['bun', 'run', 'desktop:build:debug'])
run([
  'cargo',
  'test',
  '--manifest-path',
  'src-tauri/Cargo.toml',
  '-p',
  'pathkeep-desktop',
  '--lib',
  'updater',
])
run([
  'cargo',
  'test',
  '--manifest-path',
  'src-tauri/Cargo.toml',
  '-p',
  'pathkeep-desktop',
  '--lib',
  'file_manager',
])
run([
  'bunx',
  'vitest',
  'run',
  'src/lib/update.test.ts',
  'src/lib/ipc/updater-progress.test.ts',
])

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
