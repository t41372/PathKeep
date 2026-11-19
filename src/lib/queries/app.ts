/**
 * Queries and mutations for app-wide state: the snapshot, the dashboard
 * summary and config saves. Every settings save goes through `saveConfig`.
 */
import {
  useMutation,
  useQuery,
  useQueryClient,
  type QueryClient,
} from '@tanstack/react-query'
import { appClient } from '@/lib/backend-client/app'
import { dashboardClient } from '@/lib/backend-client/dashboard'
import { settingsSaved } from '@/lib/ipc/model-download'
import { queryKeys } from '@/lib/query'
import type { AppConfig, AppSnapshot } from '@/lib/types'

/**
 * The snapshot is loaded by the session during boot and kept in the cache.
 * Screens read it from here; they should not trigger their own reloads.
 */
export function useSnapshot() {
  const query = useQuery({
    queryKey: queryKeys.snapshot,
    queryFn: appClient.getSnapshot,
    staleTime: Infinity,
  })
  return query.data as AppSnapshot
}

export function useDashboard() {
  return useQuery({
    queryKey: queryKeys.dashboard,
    queryFn: dashboardClient.getSnapshot,
  })
}

/**
 * Saves the settings: every save goes through here, so what the backend does
 * on any save is shown once, wherever the save came from.
 * - It can answer that the new settings are in effect but the disk did not
 *   confirm they will survive a power loss (`configDurabilityWarning`); the
 *   save still succeeded, so it resolves, and `onConfigNotDurable` listeners
 *   hear the diagnostic. The snapshot it resolves with leaves the warning out.
 * - Settings that no longer want the built-in model stop its download; the
 *   screens say it is stopping (`settingsSaved`).
 */
export async function saveConfig(
  config: AppConfig,
  baseConfig?: AppConfig | null,
): Promise<AppSnapshot> {
  const { configDurabilityWarning, ...applied } = await appClient.saveConfig(
    config,
    baseConfig,
  )
  settingsSaved(applied.config.ai)
  if (configDurabilityWarning)
    for (const listener of notDurable) listener(configDurabilityWarning)
  return applied
}

const notDurable = new Set<(diagnostic: string) => void>()

/** Hears each save the disk did not confirm; returns a function that stops listening. */
export function onConfigNotDurable(listener: (diagnostic: string) => void) {
  notDurable.add(listener)
  return () => {
    notDurable.delete(listener)
  }
}

/**
 * Saves a config change. `save_config` merges against `baseConfig`, so a slow
 * save cannot overwrite an unrelated setting changed meanwhile, and it returns
 * the fresh snapshot, which replaces the cached one without another round trip.
 */
export function useSaveConfig() {
  const client = useQueryClient()
  return useMutation({
    mutationFn: (update: (config: AppConfig) => AppConfig) => {
      const base = currentSnapshot(client).config
      return saveConfig(update(structuredClone(base)), base)
    },
    onSuccess: (snapshot) => client.setQueryData(queryKeys.snapshot, snapshot),
  })
}

export function currentSnapshot(client: QueryClient) {
  const snapshot = client.getQueryData<AppSnapshot>(queryKeys.snapshot)
  if (!snapshot) throw new Error('The app snapshot has not loaded yet.')
  return snapshot
}
