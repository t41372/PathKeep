import { spawn, spawnSync } from 'node:child_process'

const platform = process.platform

await runHostRustTests(platform)

async function runHostRustTests(currentPlatform) {
  switch (currentPlatform) {
    case 'darwin':
      run([
        'cargo',
        'test',
        '--manifest-path',
        'src-tauri/Cargo.toml',
        '-p',
        'vault-platform',
        '--test',
        'native_host',
        '--',
        '--ignored',
        '--test-threads=1',
      ])
      return
    case 'linux':
      runLinuxNativeTests()
      return
    case 'win32':
      run([
        'cargo',
        'test',
        '--manifest-path',
        'src-tauri/Cargo.toml',
        '-p',
        'vault-platform',
        '--test',
        'native_host',
        '--',
        '--ignored',
        '--test-threads=1',
      ])
      // The archive's crash recovery (restore, import, encryption change, "Delete all data",
      // the first bring-up of an older archive) rests on how the file system renames and
      // removes. Windows does both differently (a file another program holds open cannot be
      // renamed or deleted), and no other gate runs these tests on Windows.
      await runShowingCrashedTests([
        '-p',
        'vault-core',
        '-p',
        'vault-worker',
        '--no-fail-fast',
      ])
      return
    default:
      console.log(
        `Skipping platform-native Rust tests on unsupported host: ${currentPlatform}`,
      )
  }
}

function runLinuxNativeTests() {
  const innerScript = [
    'set -euo pipefail',
    'if ! command -v dbus-run-session >/dev/null 2>&1; then echo "dbus-run-session is required for Linux native platform tests" >&2; exit 1; fi',
    'if ! command -v gnome-keyring-daemon >/dev/null 2>&1; then echo "gnome-keyring-daemon is required for Linux native platform tests" >&2; exit 1; fi',
    'export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-$(mktemp -d)}"',
    'eval "$(printf \'pathkeep-test-passphrase\' | gnome-keyring-daemon --unlock --components=secrets)"',
    'cargo test --manifest-path src-tauri/Cargo.toml -p vault-platform --test native_host -- --ignored --test-threads=1',
  ].join('; ')

  run(['dbus-run-session', '--', 'bash', '-lc', innerScript])
}

/**
 * Runs the workspace's `cargo test` with `args`. A test binary that dies (an abort, a crash) takes the output
 * it had captured with it, and the log shows neither which test was running nor what it said.
 * Each such binary is run again one test at a time, its output not captured, so the log ends
 * with the test that stopped it. The run fails either way.
 */
async function runShowingCrashedTests(args) {
  const manifest = ['--manifest-path', 'src-tauri/Cargo.toml']
  const child = spawn('cargo', ['test', ...manifest, ...args], {
    stdio: ['inherit', 'inherit', 'pipe'],
    env: process.env,
  })
  let cargoErrors = ''
  child.stderr.setEncoding('utf8')
  child.stderr.on('data', (text) => {
    process.stderr.write(text)
    cargoErrors += text
  })
  const status = await new Promise((resolve) => child.on('close', resolve))
  if (status === 0) {
    return
  }
  for (const target of crashedTargets(cargoErrors)) {
    console.error(
      `\n${target.join(' ')} stopped before it reported its tests. Running it again, one test at a time, with their output:\n`,
    )
    spawnSync(
      'cargo',
      ['test', ...manifest, ...target, '--', '--test-threads=1', '--nocapture'],
      { stdio: 'inherit', env: { ...process.env, RUST_BACKTRACE: '1' } },
    )
  }
  process.exit(status ?? 1)
}

/**
 * The test binaries that died, as cargo's own rerun hint names them
 * (`-p vault-worker --test vault_lifecycle`). Cargo names every binary that failed, and adds
 * how it exited when it did not exit as one whose tests failed (101).
 */
function crashedTargets(cargoErrors) {
  const failed =
    /to rerun pass `([^`]+)`\s+Caused by:\s+process didn't exit successfully: `[^`]+` \(exit code: ([^,)]+)/g
  return [...cargoErrors.matchAll(failed)]
    .filter(([, , code]) => code.trim() !== '101')
    .map(([, target]) => target.split(' '))
}

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
