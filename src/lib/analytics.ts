import { isTauri } from '@tauri-apps/api/core'
import type {
  AnalyticsConfig,
  AnalyticsEvent,
  AppBuildInfo,
  LanguagePreference,
} from './types'

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

interface AnalyticsRuntime {
  endpoint: string | null
  isDesktop: boolean
  isProduction: boolean
  fetchImpl: typeof fetch | null
  now: () => string
}

function resolveAnalyticsEndpoint(endpoint: unknown): string | null {
  if (typeof endpoint !== 'string') {
    return null
  }

  const trimmed = endpoint.trim()
  return trimmed.length > 0 ? trimmed : null
}

export const CONFIGURED_ANALYTICS_ENDPOINT = resolveAnalyticsEndpoint(
  import.meta.env.VITE_ANALYTICS_ENDPOINT,
)

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

function defaultRuntime(): AnalyticsRuntime {
  return {
    endpoint: CONFIGURED_ANALYTICS_ENDPOINT,
    isDesktop: isTauri(),
    isProduction: import.meta.env.PROD,
    fetchImpl: typeof fetch === 'function' ? fetch.bind(globalThis) : null,
    now: () => new Date().toISOString(),
  }
}

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
