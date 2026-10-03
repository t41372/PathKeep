/**
 * Queries and mutations for app-wide state: the snapshot, the dashboard
 * summary and config saves.
 */
import {
  useMutation,
  useQuery,
  useQueryClient,
  type QueryClient,
} from '@tanstack/react-query'
import { appClient } from '@/lib/backend-client/app'
import { dashboardClient } from '@/lib/backend-client/dashboard'
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
 * Saves a config change. `save_config` merges against `baseConfig`, so a slow
 * save cannot overwrite an unrelated setting changed meanwhile, and it returns
 * the fresh snapshot, which replaces the cached one without another round trip.
 */
export function useSaveConfig() {
  const client = useQueryClient()
  return useMutation({
    mutationFn: (update: (config: AppConfig) => AppConfig) => {
      const base = currentSnapshot(client).config
      return appClient.saveConfig(update(structuredClone(base)), base)
    },
    onSuccess: (snapshot) => client.setQueryData(queryKeys.snapshot, snapshot),
  })
}

export function currentSnapshot(client: QueryClient) {
  const snapshot = client.getQueryData<AppSnapshot>(queryKeys.snapshot)
  if (!snapshot) throw new Error('The app snapshot has not loaded yet.')
  return snapshot
}
