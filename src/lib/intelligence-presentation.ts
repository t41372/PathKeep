import type { EnrichmentPluginStatus, IntelligenceJobOverview } from './types'

type JobsTranslator = (
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
    const suffix = `${url.pathname}${url.search}${url.hash}` || '/'

    if (hostname.length >= maxLength - 1) {
      return compactMiddle(hostname, maxLength)
    }

    const remaining = Math.max(12, maxLength - hostname.length - 1)
    return `${hostname}/${compactMiddle(
      suffix.replace(/^\//, '') || '/',
      remaining,
    )}`
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
    const url = source.url?.toLowerCase() ?? ''
    if (url.endsWith('.pdf') || url.includes('/pdf/')) {
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

  const maximumFractionDigits = percentage > 0 && percentage < 10 ? 1 : 0
  const formatter = new Intl.NumberFormat(language, {
    maximumFractionDigits,
    minimumFractionDigits:
      maximumFractionDigits === 1 && !Number.isInteger(percentage) ? 1 : 0,
  })

  return `${formatter.format(percentage)}%`
}

export function runtimeJobMutationNeedsRefresh(message: string) {
  return /cannot be (cancelled|retried)/i.test(message)
}
