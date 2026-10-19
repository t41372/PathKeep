/**
 * Records what every thread of the app was doing when a test failed.
 *
 * A step that hangs (a command that never answers while others do) leaves a
 * trace that only says it waited. The app is still running when the test
 * ends, so its threads show what the command waits on: a lock, another
 * command, a file. Written to `artifacts/e2e/thread-stacks/`.
 *
 * Responsible for: finding the app's processes and saving their stacks.
 * Not responsible for: deciding why a test failed.
 */
import type { Reporter, TestCase, TestResult } from '@playwright/test/reporter'
import { spawnSync } from 'node:child_process'
import { mkdirSync, rmSync, writeFileSync } from 'node:fs'
import path from 'node:path'

const out = 'artifacts/e2e/thread-stacks'

/**
 * The desktop app and its worker sidecar, as the dev bridge runs them: only
 * executables built for this run (in this checkout or the E2E build folder),
 * never an installed PathKeep someone has open on the same computer.
 */
function appProcesses() {
  const ours = [
    process.cwd(),
    process.env.CARGO_TARGET_DIR ?? process.cwd(),
  ].map((dir) => path.resolve(dir) + path.sep)
  const listed = spawnSync('ps', ['-axww', '-o', 'pid=,command='], {
    encoding: 'utf8',
  })
  return listed.stdout
    .split('\n')
    .map((line) => /^\s*(\d+)\s+(\S+)/.exec(line))
    .filter((match): match is RegExpExecArray => match !== null)
    .filter(
      ([, , executable]) =>
        /\/pathkeep-(desktop|worker)$/.test(executable) &&
        ours.some((dir) => executable.startsWith(dir)),
    )
    .map(([, pid]) => Number(pid))
}

/** All threads' stacks, with the tool each platform has. */
function stacks(pid: number) {
  const [command, args] =
    process.platform === 'darwin'
      ? ['sample', [String(pid), '3']]
      : ['gdb', ['-p', String(pid), '-batch', '-ex', 'thread apply all bt']]
  const run = spawnSync(command, args, { encoding: 'utf8', timeout: 60_000 })
  if (run.error) return `${command} could not run: ${run.error.message}\n`
  return `$ ${command} ${args.join(' ')}\n${run.stdout}\n${run.stderr}`
}

export default class ThreadStacksReporter implements Reporter {
  /** A run's stacks are its own, as Playwright's test results are. */
  onBegin() {
    rmSync(out, { recursive: true, force: true })
  }

  onTestEnd(test: TestCase, result: TestResult) {
    if (result.status !== 'failed' && result.status !== 'timedOut') return
    if (process.platform === 'win32') return
    const name = test
      .titlePath()
      .slice(1)
      .join(' › ')
      .replace(/[^\w.-]+/g, '-')
      .slice(0, 120)
    mkdirSync(out, { recursive: true })
    for (const pid of appProcesses()) {
      const file = path.join(out, `${name}-${pid}.txt`)
      writeFileSync(file, stacks(pid))
      console.log(`Thread stacks of process ${pid}: ${file}`)
    }
  }
}
