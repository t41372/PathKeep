/**
 * Entity and drilldown-focused Core Intelligence API wrappers.
 *
 * Why this file exists:
 * - M10 keeps route-first entity surfaces together while separating them from
 *   overview caching and runtime/output transport helpers.
 */

import { call } from '../../backend-client/shared'
import type {
  CompareSetDetail,
  DateRange,
  DayInsights,
  DomainDeepDive,
  DomainTrend,
  Explanation,
  HubPage,
  NavigationPath,
  PaginationParams,
  QueryFamilyDetail,
  RefindExplanation,
  RefindPageDetail,
  SessionDetail,
  SessionListResult,
  TrailDetail,
  TrailListResult,
} from '../types'
import { invokeRequest, invokeSectionRequest } from './shared'

export function getDomainTrend(domain: string, dateRange: DateRange) {
  return invokeRequest<
    DomainTrend,
    {
      registrableDomain: string
      dateRange: DateRange
    }
  >('get_domain_trend', { registrableDomain: domain, dateRange })
}

export function getQueryFamilyDetail(
  familyId: string,
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeSectionRequest<
    QueryFamilyDetail,
    {
      familyId: string
      dateRange: DateRange
      profileId?: string | null
    }
  >(
    'get_query_family_detail',
    {
      familyId,
      dateRange,
      profileId,
    },
    'query-family-detail',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function getRefindPageDetail(
  canonicalUrl: string,
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeSectionRequest<
    RefindPageDetail,
    {
      canonicalUrl: string
      dateRange: DateRange
      profileId?: string | null
    }
  >(
    'get_refind_page_detail',
    {
      canonicalUrl,
      dateRange,
      profileId,
    },
    'refind-page-detail',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function explainRefind(canonicalUrl: string) {
  return invokeRequest<RefindExplanation, { canonicalUrl: string }>(
    'explain_refind',
    { canonicalUrl },
  )
}

export function getSessions(
  dateRange: DateRange,
  profileId?: string | null,
  pagination?: PaginationParams,
) {
  return invokeRequest<
    SessionListResult,
    {
      dateRange: DateRange
      profileId?: string | null
      page: number
      pageSize: number
    }
  >('get_sessions', {
    dateRange,
    profileId,
    page: pagination?.page ?? 0,
    pageSize: pagination?.pageSize ?? 20,
  })
}

export function getSessionDetail(sessionId: string) {
  return call<SessionDetail>('get_session_detail', { sessionId })
}

export function getSearchTrails(
  dateRange: DateRange,
  profileId?: string | null,
  engine?: string,
  pagination?: PaginationParams,
) {
  return invokeRequest<
    TrailListResult,
    {
      dateRange: DateRange
      profileId?: string | null
      engine?: string
      page: number
      pageSize: number
    }
  >('get_search_trails', {
    dateRange,
    profileId,
    engine,
    page: pagination?.page ?? 0,
    pageSize: pagination?.pageSize ?? 20,
  })
}

export function getTrailDetail(trailId: string) {
  return call<TrailDetail>('get_trail_detail', { trailId })
}

export function getNavigationPath(visitId: number) {
  return call<NavigationPath>('get_navigation_path', { visitId })
}

export function getHubPages(
  dateRange: DateRange,
  profileId?: string | null,
  limit?: number,
) {
  return invokeRequest<
    HubPage[],
    {
      dateRange: DateRange
      profileId?: string | null
      limit?: number
    }
  >('get_hub_pages', {
    dateRange,
    profileId,
    limit,
  })
}

export function getDomainDeepDive(
  domain: string,
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeSectionRequest<
    DomainDeepDive,
    {
      registrableDomain: string
      dateRange: DateRange
      profileId?: string | null
    }
  >(
    'get_domain_deep_dive',
    {
      registrableDomain: domain,
      dateRange,
      profileId,
    },
    'domain-deep-dive',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function getDayInsights(date: string, profileId?: string | null) {
  return invokeSectionRequest<
    DayInsights,
    {
      date: string
      profileId?: string | null
    }
  >(
    'get_day_insights',
    {
      date,
      profileId,
    },
    'day-insights',
    {
      kind: 'date-range',
      dateRange: {
        start: date,
        end: date,
      },
    },
  )
}

export function getCompareSetDetail(
  compareSetId: string,
  dateRange: DateRange,
  profileId?: string | null,
) {
  return invokeSectionRequest<
    CompareSetDetail,
    {
      compareSetId: string
      dateRange: DateRange
      profileId?: string | null
    }
  >(
    'get_compare_set_detail',
    {
      compareSetId,
      dateRange,
      profileId,
    },
    'compare-set-detail',
    {
      kind: 'date-range',
      dateRange,
    },
  )
}

export function explainEntity(entityType: string, entityId: string) {
  return invokeRequest<Explanation, { entityType: string; entityId: string }>(
    'explain_entity',
    {
      entityType,
      entityId,
    },
  )
}
