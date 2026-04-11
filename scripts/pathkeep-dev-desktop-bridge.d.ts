export interface DesktopBridgeEnv {
  devServerPort: number
  devServerUrl: string
  devIpcPort: number
  devIpcUrl: string
}

export function resolveDesktopBridgeEnv(
  env?: NodeJS.ProcessEnv,
): DesktopBridgeEnv
