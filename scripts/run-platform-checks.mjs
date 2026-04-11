import { spawnSync } from 'node:child_process'

run(['bun', 'run', 'test:platform:rust'])
run(['bun', 'run', 'test:platform:desktop'])

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
