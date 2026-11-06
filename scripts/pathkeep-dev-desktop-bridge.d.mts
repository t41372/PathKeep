export interface DesktopBridgeEnv {
  devServerPort: number
  devServerUrl: string
  devIpcPort: number
  devIpcUrl: string
}

export function resolveDesktopBridgeEnv(
  env?: NodeJS.ProcessEnv,
): DesktopBridgeEnv

/** The Cargo target folder the E2E configurations and the demo build in. */
export function e2eCargoTargetDir(env?: NodeJS.ProcessEnv): string
