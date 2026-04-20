/**
 * Runtime review and reusable output Core Intelligence API wrappers.
 *
 * Why this file exists:
 * - External-output payload providers and local-host artifact flows share one
 *   boundary after M10, separate from route/entity read models.
 */

import type {
  DateRange,
  IntelligenceEmbedCardPayload,
  IntelligenceLocalHostBuildResult,
  IntelligenceLocalHostPreview,
  IntelligencePublicSnapshot,
  IntelligenceWidgetSnapshot,
} from '../types'
import { invokeRequest } from './shared'

export function getIntelligenceEmbedCards(
  dateRange: DateRange,
  profileId?: string | null,
  limit?: number,
) {
  return invokeRequest<
    IntelligenceEmbedCardPayload[],
    {
      dateRange: DateRange
      profileId?: string | null
      limit?: number
    }
  >('get_intelligence_embed_cards', {
    dateRange,
    profileId,
    limit,
  })
}

export function getIntelligenceWidgetSnapshot(
  dateRange: DateRange,
  profileId?: string | null,
  limit?: number,
) {
  return invokeRequest<
    IntelligenceWidgetSnapshot,
    {
      dateRange: DateRange
      profileId?: string | null
      limit?: number
    }
  >('get_intelligence_widget_snapshot', {
    dateRange,
    profileId,
    limit,
  })
}

export function getIntelligencePublicSnapshot(
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeRequest<
    IntelligencePublicSnapshot,
    {
      dateRange: DateRange
      profileId?: string | null
    }
  >('get_intelligence_public_snapshot', {
    dateRange,
    profileId,
  })
}

export function previewIntelligenceLocalHost(
  dateRange: DateRange,
  locale: string,
  profileId?: string | null,
) {
  return invokeRequest<
    IntelligenceLocalHostPreview,
    {
      dateRange: DateRange
      profileId?: string | null
      locale: string
    }
  >('preview_intelligence_local_host', {
    dateRange,
    profileId,
    locale,
  })
}

export function buildIntelligenceLocalHost(
  dateRange: DateRange,
  locale: string,
  profileId?: string | null,
) {
  return invokeRequest<
    IntelligenceLocalHostBuildResult,
    {
      dateRange: DateRange
      profileId?: string | null
      locale: string
    }
  >('build_intelligence_local_host', {
    dateRange,
    profileId,
    locale,
  })
}
