#!/usr/bin/env node

import { spawn } from 'node:child_process'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const playwrightCliPath = require.resolve('@playwright/test/cli')

const env = { ...process.env }

// Node warns when either downstream tool reintroduces the other flag, so keep
// the Playwright environment neutral and let each process decide its own color
// support.
delete env.NO_COLOR
delete env.FORCE_COLOR

const child = spawn(
  process.execPath,
  [playwrightCliPath, 'test', ...process.argv.slice(2)],
  {
    stdio: 'inherit',
    env,
  },
)

child.on('exit', (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal)
    return
  }

  process.exit(code ?? 1)
})

child.on('error', (error) => {
  console.error(error)
  process.exit(1)
})
