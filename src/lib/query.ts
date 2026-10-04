/**
 * The single TanStack Query client and the query keys shared across screens.
 *
 * Every backend read goes through a query so results are cached, deduplicated
 * and refreshed by invalidating a key, instead of each screen keeping its own
 * copy and refetching on every mount. After a backup or import finishes, the
 * backup runner invalidates `archiveData`, which refreshes everything derived
 * from the archive in one pass.
 */
import { QueryClient } from '@tanstack/react-query'

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // Archive reads change only after a backup/import, which invalidates
      // explicitly. Refetching on focus would just hammer the backend.
      staleTime: 5 * 60_000,
      gcTime: 15 * 60_000,
      refetchOnWindowFocus: false,
      retry: false,
    },
  },
})

export const queryKeys = {
  snapshot: ['snapshot'] as const,
  buildInfo: ['build-info'] as const,
  lockStatus: ['lock-status'] as const,
  /** Prefix for every query whose result depends on archive contents. */
  archiveData: ['archive'] as const,
  dashboard: ['archive', 'dashboard'] as const,
  schedule: ['schedule-status'] as const,
  exportProgress: (exportId: string) => ['export-progress', exportId] as const,
}
