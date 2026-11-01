#!/usr/bin/env node

import { spawn } from 'node:child_process'
import { mkdtempSync, rmSync } from 'node:fs'
import { createRequire } from 'node:module'
import os from 'node:os'
import path from 'node:path'

const require = createRequire(import.meta.url)
const playwrightCliPath = require.resolve('@playwright/test/cli')

const args = process.argv.slice(2)
const env = { ...process.env, TZ: 'UTC' }
delete env.NO_COLOR
delete env.FORCE_COLOR

// Each configuration starts a separate real backend and fixture. Explicit CLI
// arguments remain a targeted run; the authority gate runs every configuration.
const runs = args.length
  ? [{ args, variant: env.PATHKEEP_BACKUP_INTEGRITY_VARIANT }]
  : [
      { args: [], artifact: 'artifacts/e2e' },
      {
        args: ['--config', 'playwright.backup-integrity.config.ts'],
        variant: 'healthy',
        artifact: 'artifacts/e2e/backup-integrity-healthy',
      },
      {
        args: ['--config', 'playwright.backup-integrity.config.ts'],
        variant: 'damaged',
        artifact: 'artifacts/e2e/backup-integrity-damaged',
      },
    ]
// Each run's fixture (browsers, archive, keyring: 100–500 MB) goes in a folder
// of its own, deleted once the run passed; a failed run keeps it to look into.
function run(config) {
  const root = mkdtempSync(path.join(os.tmpdir(), 'pathkeep-e2e-'))
  return new Promise((resolve) => {
    const child = spawn(
      process.execPath,
      [playwrightCliPath, 'test', ...config.args],
      {
        stdio: 'inherit',
        env: {
          ...env,
          PATHKEEP_E2E_ROOT: root,
          ...(config.variant
            ? { PATHKEEP_BACKUP_INTEGRITY_VARIANT: config.variant }
            : {}),
        },
      },
    )
    child.on('error', (error) => {
      console.error(error)
      resolve(1)
    })
    child.on('exit', (code, signal) => {
      const artifact =
        config.artifact ??
        (config.args.some((arg) => arg.includes('backup-integrity'))
          ? `artifacts/e2e/backup-integrity-${config.variant ?? 'healthy'}`
          : 'artifacts/e2e')
      console.log(
        `E2E artifacts: ${artifact}/ (report/index.html, results.json, test-results/)`,
      )
      if (code === 0) rmSync(root, { recursive: true, force: true })
      else console.log(`E2E fixture kept: ${root}`)
      if (signal) {
        process.kill(process.pid, signal)
        resolve(1)
      } else {
        resolve(code ?? 1)
      }
    })
  })
}
for (const config of runs) {
  const code = await run(config)
  if (code !== 0) {
    process.exitCode = code
    break
  }
}
