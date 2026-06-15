import type { EnrichmentPluginStatus, IntelligenceJobOverview } from './types'

/**
 * Defines the translator contract shared by Jobs-specific runtime presentation helpers.
 *
 * These helpers intentionally stay route-aware because they translate queue and
 * plugin failure states into user-facing Jobs copy. Exporting the type keeps
 * other Jobs-owned modules aligned without forcing them to duplicate the
 * callback signature locally.
 */
export type JobsTranslator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

type RuntimeErrorSource = Pick<IntelligenceJobOverview, 'lastError' | 'url'>

function compactMiddle(text: string, maxLength: number) {
  if (text.length <= maxLength) return text

  const head = Math.max(8, Math.floor((maxLength - 1) / 2))
  const tail = Math.max(6, maxLength - head - 1)
  return `${text.slice(0, head)}…${text.slice(-tail)}`
}

function compactUrlText(text: string, maxLength: number) {
  try {
    const url = new URL(text)
    const hostname = url.hostname.replace(/^www\./, '')
    const suffix = `${url.pathname}${url.search}${url.hash}`

    if (hostname.length >= maxLength - 1) {
      return compactMiddle(hostname, maxLength)
    }

    const remaining = Math.max(12, maxLength - hostname.length - 1)
    const readableSuffix = suffix.slice(1)
    return `${hostname}/${compactMiddle(readableSuffix || '/', remaining)}`
  } catch {
    return compactMiddle(text, maxLength)
  }
}

export function hostnameFromUrl(url?: string | null) {
  if (!url) return null
  try {
    return new URL(url).hostname.replace(/^www\./, '')
  } catch {
    return null
  }
}

export function summarizeRuntimeJobError(
  source: RuntimeErrorSource,
  jobsT: JobsTranslator,
) {
  if (!source.lastError) return jobsT('noErrorDetails')

  if (/unsupported-content/i.test(source.lastError)) {
    const url = source.url?.toLowerCase()
    if (url?.endsWith('.pdf') || url?.includes('/pdf/')) {
      return jobsT('errorPdf')
    }
    return jobsT('errorUnsupportedContent')
  }

  if (/redirect/i.test(source.lastError)) {
    return jobsT('errorRedirectBlocked')
  }

  if (/429/i.test(source.lastError)) {
    return jobsT('errorRateLimited')
  }

  return source.lastError
}

export function summarizePluginError(
  plugin: Pick<EnrichmentPluginStatus, 'lastError'> & { url?: string | null },
  jobsT: JobsTranslator,
) {
  return summarizeRuntimeJobError(
    { lastError: plugin.lastError, url: plugin.url ?? null },
    jobsT,
  )
}

export function summarizeRuntimeJob(
  job: IntelligenceJobOverview,
  jobsT: JobsTranslator,
) {
  if (job.state === 'failed') {
    return summarizeRuntimeJobError(job, jobsT)
  }

  if (job.jobType === 'deterministic-rebuild') {
    return job.progressDetail ?? jobsT('deterministicRuntimeSummary')
  }

  if (job.pluginId === 'readable-content-refetch') {
    const host = hostnameFromUrl(job.url)
    if (job.state === 'queued') {
      return host
        ? jobsT('contentFetchQueuedSummaryHost', { host })
        : jobsT('contentFetchQueuedSummary')
    }
    if (job.state === 'running') {
      return host
        ? jobsT('contentFetchRunningSummaryHost', { host })
        : jobsT('contentFetchRunningSummary')
    }
  }

  return job.title ?? job.url ?? job.lastError ?? jobsT('noErrorDetails')
}

export function compactInsightText(text: string, maxLength = 96) {
  const trimmed = text.trim()
  if (!trimmed || trimmed.length <= maxLength) return trimmed

  if (/^https?:\/\//i.test(trimmed)) {
    return compactUrlText(trimmed, maxLength)
  }

  return compactMiddle(trimmed, maxLength)
}

export function formatInsightCoverage(
  contentCoverage: number,
  language: string,
) {
  const percentage = contentCoverage * 100
  if (percentage > 0 && percentage < 1) {
    return '<1%'
  }

  // Stryker disable next-line ConditionalExpression,EqualityOperator: zero and exactly ten format identically through Intl; adjacent threshold tests cover every visible output branch.
  const maximumFractionDigits = percentage > 0 && percentage < 10 ? 1 : 0
  const formatter = new Intl.NumberFormat(language, {
    maximumFractionDigits,
  })

  return `${formatter.format(percentage)}%`
}

export function runtimeJobMutationNeedsRefresh(message: string) {
  return /cannot be (cancelled|retried)/i.test(message)
}

/**
 * Reports whether an AI-queue retry/cancel rejection is a stale-state race
 * rather than a genuine failure worth alarming the user about.
 *
 * Why this exists: the Jobs panels render a queue snapshot, so a row's state
 * can change (queued → running, failed → retried elsewhere) between paint and
 * the user's click. When that happens the backend rejects the mutation with a
 * deterministic "state no longer allows this" message, and the honest response
 * is to silently re-read truth instead of surfacing a scary banner. The AI
 * queue phrases these races differently from the deterministic runtime queue —
 * replay says "Only ... can be replayed." and cancel says "... cannot be
 * cancelled." — so it gets its own matcher kept honest to the actual backend
 * wording (`vault-core::ai_queue`) rather than a broad regex that could swallow
 * unrelated provider errors. Any other rejection (quota, provider, IO) is a
 * real failure and must be shown.
 */
export function aiJobMutationNeedsRefresh(message: string) {
  return /cannot be cancelled|can be replayed/i.test(message)
}
