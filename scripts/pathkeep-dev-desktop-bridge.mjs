import { spawn } from 'node:child_process'
import { existsSync, rmSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

export function resolveDesktopBridgeEnv(env = process.env) {
  const devServerPort = Number(env.PATHKEEP_DEV_SERVER_PORT || 1420)
  const devIpcPort = Number(env.PATHKEEP_DEV_IPC_PORT || 43117)

  return {
    devServerPort,
    devServerUrl: `http://127.0.0.1:${devServerPort}`,
    devIpcPort,
    devIpcUrl: `http://127.0.0.1:${devIpcPort}`,
  }
}

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
)

/**
 * Where the E2E configurations and the demo build the desktop app. One folder
 * kept between runs: a cold Rust build takes minutes, and a gate that runs
 * several configurations builds the app once, not once per configuration.
 */
export function e2eCargoTargetDir(env = process.env) {
  return (
    env.CARGO_TARGET_DIR ??
    path.join(repoRoot, 'var', 'playwright', 'desktop-bridge', 'cargo-target')
  )
}

const scriptPath = fileURLToPath(import.meta.url)

if (process.argv[1] === scriptPath) {
  const resolved = resolveDesktopBridgeEnv()
  const env = {
    ...process.env,
    PATHKEEP_ENABLE_DEV_IPC_BRIDGE: '1',
    PATHKEEP_DEV_SERVER_PORT: String(resolved.devServerPort),
    PATHKEEP_DEV_IPC_PORT: String(resolved.devIpcPort),
    PATHKEEP_DEV_IPC_ALLOWED_ORIGINS:
      process.env.PATHKEEP_DEV_IPC_ALLOWED_ORIGINS ||
      `${resolved.devServerUrl},http://localhost:${resolved.devServerPort}`,
    VITE_PATHKEEP_DEV_IPC_URL: resolved.devIpcUrl,
  }

  const tauriConfigOverride = JSON.stringify({
    build: {
      devUrl: resolved.devServerUrl,
    },
  })

  console.log(
    `PathKeep desktop bridge enabled at ${resolved.devIpcUrl}. Open ${resolved.devServerUrl} in Chrome or run bun run test:e2e.`,
  )

  if (env.PATHKEEP_E2E_RESTART_FILE) {
    await superviseForE2E(
      env,
      tauriConfigOverride,
      env.PATHKEEP_E2E_RESTART_FILE,
    )
  } else {
    runTauriDev(env, tauriConfigOverride)
  }
}

/** `tauri dev`, which builds and runs the app and the frontend's dev server. */
function runTauriDev(env, tauriConfigOverride) {
  const child = spawn(
    'bun',
    [
      'x',
      'tauri',
      'dev',
      '--features',
      'devtools-bridge',
      '--config',
      tauriConfigOverride,
    ],
    {
      stdio: 'inherit',
      env,
    },
  )

  child.on('error', (error) => {
    console.error(`PathKeep could not start the dev desktop bridge: ${error}`)
    process.exit(1)
  })

  child.on('exit', (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal)
      return
    }

    process.exit(code ?? 0)
  })
}

/**
 * For the E2E run (`PATHKEEP_E2E_RESTART_FILE`): does what `tauri dev` does —
 * the worker sidecar, Vite, one build of the app — but runs the app itself, so
 * a spec can kill it the way a crash would and get it back. When the app exits
 * and the restart file exists, the file is removed and the app starts again on
 * the same ports and data; any other exit ends the run, as with `tauri dev`.
 */
async function superviseForE2E(env, tauriConfigOverride, restartFile) {
  const run = (command, args, extraEnv = {}) =>
    new Promise((resolve, reject) => {
      const child = spawn(command, args, {
        stdio: 'inherit',
        env: { ...env, ...extraEnv },
      })
      child.on('error', reject)
      child.on('exit', (code) =>
        code === 0
          ? resolve()
          : reject(
              new Error(`${command} ${args.join(' ')} exited with ${code}`),
            ),
      )
    })
  rmSync(restartFile, { force: true })
  // `tauri dev` builds the sidecar in debug (it sets TAURI_ENV_DEBUG); so does this.
  await run('bun', ['run', 'prepare:worker-sidecar', '--profile', 'debug'])
  const vite = spawn('bun', ['run', 'dev'], { stdio: 'inherit', env })
  await run(
    'cargo',
    [
      'build',
      '--manifest-path',
      'src-tauri/Cargo.toml',
      '--features',
      'devtools-bridge',
    ],
    { TAURI_CONFIG: tauriConfigOverride },
  )
  const target = path.resolve(env.CARGO_TARGET_DIR ?? 'src-tauri/target')
  const binary = path.join(
    target,
    'debug',
    process.platform === 'win32' ? 'pathkeep-desktop.exe' : 'pathkeep-desktop',
  )

  let app = null
  for (const signal of ['SIGINT', 'SIGTERM']) {
    process.on(signal, () => {
      vite.kill(signal)
      app?.kill(signal)
      process.exit(1)
    })
  }
  const start = () => {
    app = spawn(binary, [], { stdio: 'inherit', env })
    app.on('exit', (code, signal) => {
      if (existsSync(restartFile)) {
        rmSync(restartFile, { force: true })
        console.log(
          'PathKeep E2E: the app stopped as a spec asked; starting it again.',
        )
        start()
        return
      }
      vite.kill()
      process.exit(code ?? (signal ? 1 : 0))
    })
  }
  start()
}
