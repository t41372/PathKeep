/**
 * @file overview-read.ts
 * @description Barrel module that preserves the public read-helper path for overview-backed Core Intelligence section reads.
 * @module lib/core-intelligence/api
 *
 * ## 職責
 * - 重新導出 primary/secondary overview-backed `get*` wrappers。
 * - 為跨 primary/secondary cache 邊界的少數 API 保留單一 owner。
 *
 * ## 不負責
 * - 不實作 read logic 本身。
 * - 不提供 load 或 peek helpers。
 */

import type { DateRange, DiscoveryTrend } from '../types'
import {
  cachedPrimaryOverview,
  cachedSecondaryOverview,
  invokeSectionRequest,
} from './shared'

export * from './overview-read-primary'
export * from './overview-read-secondary'

/**
 * Returns discovery trend, reusing the day or week overview cache when possible.
 *
 * This read spans both overview halves, so it keeps a single owner here
 * instead of duplicating the export in two submodules.
 */
export function getDiscoveryTrend(
  dateRange: DateRange,
  profileId?: string | null,
  granularity?: string,
) {
  const cached =
    granularity === 'day'
      ? cachedPrimaryOverview(dateRange, profileId)?.discoveryTrendDay
      : granularity === 'week' || granularity === undefined
        ? cachedSecondaryOverview(dateRange, profileId)?.discoveryTrendWeek
        : null
  if (cached) {
    return Promise.resolve(cached)
  }
  return invokeSectionRequest<
    DiscoveryTrend,
    { dateRange: DateRange; profileId?: string | null; granularity?: string }
  >(
    'get_discovery_trend',
    { dateRange, profileId, granularity },
    'discovery-trend',
    { kind: 'date-range', dateRange },
  )
}
