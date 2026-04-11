/**
 * This module implements the narrow, consented analytics boundary approved for the shipped front-end.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `CONFIGURED_ANALYTICS_ENDPOINT`
 * - `shouldSendAnalytics`
 * - `buildAnalyticsPayload`
 * - `trackAnalyticsEvent`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

import { isTauri } from '@tauri-apps/api/core'
import type {
  AnalyticsConfig,
  AnalyticsEvent,
  AppBuildInfo,
  LanguagePreference,
} from './types'

/**
 * Defines the typed shape for analytics payload.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
interface AnalyticsPayload {
  type: AnalyticsEvent['type']
  occurredAt: string
  appVersion: string
  screen?: string
  route?: string
  language?: LanguagePreference
  action?: string
  feature?: string
  status?: string
  version?: string | null
}

/**
 * Defines the typed shape for analytics runtime.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
interface AnalyticsRuntime {
  endpoint: string | null
  isDesktop: boolean
  isProduction: boolean
  fetchImpl: typeof fetch | null
  now: () => string
}

/**
 * Resolves analytics endpoint from the available inputs.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
function resolveAnalyticsEndpoint(endpoint: unknown): string | null {
  if (typeof endpoint !== 'string') {
    return null
  }

  const trimmed = endpoint.trim()
  return trimmed.length > 0 ? trimmed : null
}

/**
 * Exposes the shared configured analytics endpoint declaration used by this module.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export const CONFIGURED_ANALYTICS_ENDPOINT = resolveAnalyticsEndpoint(
  import.meta.env.VITE_ANALYTICS_ENDPOINT,
)

/**
 * Returns whether send analytics.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function shouldSendAnalytics(
  config: AnalyticsConfig | null | undefined,
  runtime: Pick<
    AnalyticsRuntime,
    'endpoint' | 'isDesktop' | 'isProduction' | 'fetchImpl'
  >,
) {
  return Boolean(
    config?.enabled &&
    runtime.endpoint &&
    runtime.isDesktop &&
    runtime.isProduction &&
    runtime.fetchImpl,
  )
}

/**
 * Builds analytics payload.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function buildAnalyticsPayload(
  event: AnalyticsEvent,
  buildInfo: Pick<AppBuildInfo, 'version'>,
  occurredAt: string,
): AnalyticsPayload {
  const base = {
    type: event.type,
    occurredAt,
    appVersion: buildInfo.version,
  }

  switch (event.type) {
    case 'route-view':
      return {
        ...base,
        route: event.route,
        screen: event.screen,
        language: event.language,
      }
    case 'cta-click':
      return {
        ...base,
        screen: event.screen,
        action: event.action,
        feature: event.feature,
      }
    case 'update-lifecycle':
      return {
        ...base,
        screen: event.screen,
        action: event.action,
        status: event.status,
        version: event.version ?? null,
      }
  }
}

/**
 * Returns the default runtime.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
function defaultRuntime(): AnalyticsRuntime {
  return {
    endpoint: CONFIGURED_ANALYTICS_ENDPOINT,
    isDesktop: isTauri(),
    isProduction: import.meta.env.PROD,
    fetchImpl: typeof fetch === 'function' ? fetch.bind(globalThis) : null,
    now: () => new Date().toISOString(),
  }
}

/**
 * Explains how track analytics event works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export async function trackAnalyticsEvent(
  config: AnalyticsConfig | null | undefined,
  event: AnalyticsEvent,
  buildInfo: Pick<AppBuildInfo, 'version'> | null | undefined,
  runtime: Partial<AnalyticsRuntime> = {},
) {
  if (!buildInfo) {
    return false
  }

  const resolvedRuntime = {
    ...defaultRuntime(),
    ...runtime,
  } satisfies AnalyticsRuntime

  if (!shouldSendAnalytics(config, resolvedRuntime)) {
    return false
  }

  const payload = buildAnalyticsPayload(event, buildInfo, resolvedRuntime.now())
  try {
    await resolvedRuntime.fetchImpl!(resolvedRuntime.endpoint!, {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
      },
      body: JSON.stringify(payload),
    })
    return true
  } catch {
    return false
  }
}
