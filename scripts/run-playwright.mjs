#!/usr/bin/env node

import { spawn } from 'node:child_process'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const playwrightCliPath = require.resolve('@playwright/test/cli')

const args = process.argv.slice(2)
const configFlag = args.indexOf('--config')
const configFile = configFlag === -1 ? '' : (args[configFlag + 1] ?? '')
// Mirrors the `artifactsDir` each playwright config writes to.
const artifactsDir = `artifacts/e2e/${
  configFile.match(/^playwright\.(.+)\.config\.ts$/)?.[1] ?? 'preview'
}`

const env = { ...process.env }

// Node warns when either downstream tool reintroduces the other flag, so keep
// the Playwright environment neutral and let each process decide its own color
// support.
delete env.NO_COLOR
delete env.FORCE_COLOR

const child = spawn(process.execPath, [playwrightCliPath, 'test', ...args], {
  stdio: 'inherit',
  env,
})

child.on('exit', (code, signal) => {
  console.log(
    `E2E artifacts: ${artifactsDir}/ (report/index.html, results.json, test-results/)`,
  )
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
