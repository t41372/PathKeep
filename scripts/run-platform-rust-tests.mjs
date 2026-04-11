import { spawnSync } from 'node:child_process'

const platform = process.platform

runHostRustTests(platform)

function runHostRustTests(currentPlatform) {
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
        '--test-threads=1',
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
    'cargo test --manifest-path src-tauri/Cargo.toml -p vault-platform --test native_host -- --test-threads=1',
  ].join('; ')

  run(['dbus-run-session', '--', 'bash', '-lc', innerScript])
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
