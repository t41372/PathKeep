import { spawn } from 'node:child_process'
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
    `PathKeep desktop bridge enabled at ${resolved.devIpcUrl}. Open ${resolved.devServerUrl} in Chrome or run bun run test:e2e:desktop-bridge.`,
  )

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
