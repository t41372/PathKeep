import { describe, expect, test } from 'vitest'
import {
  compactInsightText,
  formatInsightCoverage,
  hostnameFromUrl,
  runtimeJobMutationNeedsRefresh,
  summarizePluginError,
  summarizeRuntimeJob,
  summarizeRuntimeJobError,
} from './intelligence-presentation'
import type { IntelligenceJobOverview } from './types'

const jobsT = (key: string, vars?: Record<string, string | number>) => {
  switch (key) {
    case 'noErrorDetails':
      return 'No details yet.'
    case 'contentFetchQueuedSummaryHost':
      return `Queued ${vars?.host ?? 'unknown host'}`
    case 'contentFetchRunningSummaryHost':
      return `Running ${vars?.host ?? 'unknown host'}`
    case 'errorPdf':
      return 'This page returned a PDF, so PathKeep skipped readable-text extraction.'
    case 'errorUnsupportedContent':
      return 'This page did not return readable HTML, so PathKeep skipped page-text extraction.'
    case 'errorRedirectBlocked':
      return 'This page redirected into a sign-in or redirect boundary, so PathKeep could not keep a stable readable copy.'
    case 'errorRateLimited':
      return 'The upstream site temporarily refused the fetch. Retry it later.'
    default:
      return key
  }
}

describe('intelligence presentation helpers', () => {
  test('summarizes known runtime failures into honest copy', () => {
    expect(
      summarizeRuntimeJobError(
        {
          lastError: null,
          url: 'https://example.com/article',
        },
        jobsT,
      ),
    ).toBe('No details yet.')

    expect(
      summarizeRuntimeJobError(
        {
          lastError: 'unsupported-content',
          url: 'https://example.com/report.pdf',
        },
        jobsT,
      ),
    ).toBe(
      'This page returned a PDF, so PathKeep skipped readable-text extraction.',
    )
    expect(
      summarizeRuntimeJobError(
        {
          lastError: 'unsupported-content',
          url: 'https://example.com/pdf/report',
        },
        jobsT,
      ),
    ).toBe(
      'This page returned a PDF, so PathKeep skipped readable-text extraction.',
    )
    expect(
      summarizeRuntimeJobError(
        {
          lastError: 'unsupported-content',
          url: 'https://example.com/audio',
        },
        jobsT,
      ),
    ).toBe(
      'This page did not return readable HTML, so PathKeep skipped page-text extraction.',
    )
    expect(
      summarizeRuntimeJobError(
        {
          lastError: 'redirect boundary',
          url: 'https://example.com/article',
        },
        jobsT,
      ),
    ).toBe(
      'This page redirected into a sign-in or redirect boundary, so PathKeep could not keep a stable readable copy.',
    )

    expect(
      summarizeRuntimeJobError(
        {
          lastError: '429 from upstream host',
          url: 'https://example.com/article',
        },
        jobsT,
      ),
    ).toBe('The upstream site temporarily refused the fetch. Retry it later.')
    expect(
      summarizePluginError({ lastError: '429 from upstream host' }, jobsT),
    ).toBe('The upstream site temporarily refused the fetch. Retry it later.')
    expect(
      summarizePluginError(
        {
          lastError: 'unsupported-content',
          url: 'https://example.com/report.pdf',
        },
        jobsT,
      ),
    ).toBe(
      'This page returned a PDF, so PathKeep skipped readable-text extraction.',
    )
  })

  test('summarizes runtime jobs by state, plugin, and fallback detail', () => {
    expect(
      summarizeRuntimeJob(
        {
          state: 'failed',
          lastError: 'raw failure',
        } as IntelligenceJobOverview,
        jobsT,
      ),
    ).toBe('raw failure')
    expect(
      summarizeRuntimeJob(
        {
          state: 'failed',
          lastError: 'unsupported-content',
          url: 'https://example.com/report.pdf',
        } as IntelligenceJobOverview,
        jobsT,
      ),
    ).toBe(
      'This page returned a PDF, so PathKeep skipped readable-text extraction.',
    )
    expect(
      summarizeRuntimeJob(
        {
          state: 'running',
          jobType: 'deterministic-rebuild',
          progressDetail: 'Rebuilding daily rollups',
        } as IntelligenceJobOverview,
        jobsT,
      ),
    ).toBe('Rebuilding daily rollups')
    expect(
      summarizeRuntimeJob(
        {
          state: 'running',
          jobType: 'deterministic-rebuild',
          progressDetail: null,
        } as IntelligenceJobOverview,
        jobsT,
      ),
    ).toBe('deterministicRuntimeSummary')
    expect(
      summarizeRuntimeJob(
        {
          state: 'queued',
          pluginId: 'readable-content-refetch',
          url: 'https://www.example.com/path',
        } as IntelligenceJobOverview,
        jobsT,
      ),
    ).toBe('Queued example.com')
    expect(
      summarizeRuntimeJob(
        {
          state: 'queued',
          pluginId: 'readable-content-refetch',
          url: null,
        } as IntelligenceJobOverview,
        jobsT,
      ),
    ).toBe('contentFetchQueuedSummary')
    expect(
      summarizeRuntimeJob(
        {
          state: 'running',
          pluginId: 'readable-content-refetch',
          url: 'https://docs.example.com/path',
        } as IntelligenceJobOverview,
        jobsT,
      ),
    ).toBe('Running docs.example.com')
    expect(
      summarizeRuntimeJob(
        {
          state: 'running',
          pluginId: 'readable-content-refetch',
          url: 'not a url',
        } as IntelligenceJobOverview,
        jobsT,
      ),
    ).toBe('contentFetchRunningSummary')
    expect(
      summarizeRuntimeJob(
        {
          state: 'succeeded',
          pluginId: 'readable-content-refetch',
          title: 'Fetched page',
          url: 'https://docs.example.com/path',
        } as IntelligenceJobOverview,
        jobsT,
      ),
    ).toBe('Fetched page')
    expect(
      summarizeRuntimeJob(
        {
          state: 'queued',
          title: 'Readable title',
          url: 'https://example.com/fallback',
          lastError: 'unused',
        } as IntelligenceJobOverview,
        jobsT,
      ),
    ).toBe('Readable title')
    expect(
      summarizeRuntimeJob(
        {
          state: 'queued',
          title: null,
          url: 'https://example.com/fallback',
          lastError: 'unused',
        } as IntelligenceJobOverview,
        jobsT,
      ),
    ).toBe('https://example.com/fallback')
    expect(
      summarizeRuntimeJob(
        {
          state: 'queued',
          title: null,
          url: null,
          lastError: 'fallback error',
        } as IntelligenceJobOverview,
        jobsT,
      ),
    ).toBe('fallback error')
    expect(
      summarizeRuntimeJob(
        {
          state: 'queued',
          title: null,
          url: null,
          lastError: null,
        } as IntelligenceJobOverview,
        jobsT,
      ),
    ).toBe('No details yet.')
  })

  test('compacts long tracking-style URLs into readable labels', () => {
    const rawUrl =
      'https://elink2fb.mail.gethalfbaked.com/ss/c/u001.W7yY1DE5FLIFsRdzd8xlOFxIFmg-LnZyrakPeT0Kr4Akcsd_1nNyG4O-JzeKCfzCHx0L-Q-XaEfbRodxc4QPmYWsoxwYVKVdTuQECM3bSYEh-a_vxV99Ks-5wLaLiMaeY37qxnfpzNzmpXsxTvq_IPXz5HgP5iJZKdDkWjrUqimIY5PctDJFjhBu_zPRRgAg-dQNhu6BYaOyTuNS1qwtzKJBmoFK-4zVhhNl9iu13mgWvQtpcozXWzWK8THCZHcPxnrCdMIIBGV828w4PzF5yB_x3RR-pCoiYJSxvPV7JY5xBOnmLvfnv6r5NzjB43ogYyVW_4TBHTEm9p1YBYuXct8p7_gVGKhCalmuEO4l29RrpEepk_Zt8ZfJbgCHA2NjhaShq8P_ecCqoZkhZbPdK7hXCmS6vCQQRwbCqJMn7rA_6CzHp8cuu2dLM1WxsvcV1C5v3062OGqpliWy3p_B6ZqbcyyyAVhk3tEUvBYh8TUYjsNVDIOCyEAc0dkLDNoDaBX2UtwqDtlcFjtIHf7x8fmFqODD0nlBa94zHuY8gUv1PcmjeDQU_ibOvtlZSJutjF08QvPetF_tT9R1AG-pCUZYPqZPxqcyIhSnpMPA85mgUOD4Ssj59-ReWgfWvpXDFwIpILbfYzvkr6rd2QwWYR7mD2nmsRHmhGPLM4ar_Tdo8VRzL00zEoB-LSpWB_aZAa1FIBk3dWFYNrjwOwn-R6p4vxsbWIzCgtsPQEIjA8CHYwMpgFjDUz7hRPmSHIzKIE4t5jS9G3uUkMWr1m-_JqtApppz5izeYwpp8sYQOhIp02VjCNahZjKvFVAel4258O1sayVYHxyrT181U82dM1cobtRdxHUToaSFPC_2voJT-pw2NzWLA8-sy0gVhOtHAn0VGS_TSTRrJWoukJubqwM2cjTB7VvMhREDJND84eG0Lz0CeZue7gVy4dz-O1UMAy6_ghvYEslOJ8MYK71du2uV8w8d26maPf_m_b6HuuOddElBfgZjYFWwhpjNModDmZKXHamsBFP3Xt-6an2SyB51igaGpmn190V5Zvo8kwk/4po/kXT0fItVSwCLj4QagxK9mw/h35/h001.Asq1GllC9cmWzWIRdZ3K50d8fC8sPF_cT-N0wkOUle8'

    const compact = compactInsightText(rawUrl, 88)
    expect(compact).toContain('elink2fb.mail.gethalfbaked.com')
    expect(compact).toContain('…')
    expect(compact.length).toBeLessThanOrEqual(89)

    expect(
      compactInsightText(
        'https://www.averyveryveryveryveryveryverylonghostname.example/path',
        24,
      ),
    ).toContain('…')
    expect(compactInsightText('https://example.com/a', 16)).toBe(
      'example.com/a',
    )
    expect(compactInsightText('https://example.com', 16)).toBe('example.com//')
    expect(compactInsightText('abcdefghij', 10)).toBe('abcdefghij')
    expect(compactInsightText('abcdefghijklmnopqrstuvwxyz', 20)).toBe(
      'abcdefghi…qrstuvwxyz',
    )
    expect(compactInsightText('https://x.co', 12)).toBe('https://x.co')
    expect(compactInsightText('https://www.example.com/abcdefghijkl', 24)).toBe(
      'example.com/abcdefghijkl',
    )
    expect(
      compactInsightText('https://example.com/abcdefghijklmnopqrstuvwxyz', 24),
    ).toBe('example.com/abcdefgh…uvwxyz')
    expect(
      compactInsightText('https://example.com/abcdefghijklmnopqrstuvwxyz', 40),
    ).toBe('example.com/abcdefghijklmnopqrstuvwxyz')
    expect(compactInsightText('https://abcdefghijklmno/path', 16)).toBe(
      'abcdefghijklmno',
    )
    expect(compactInsightText('https://foo.www.example.com/path', 24)).toBe(
      'foo.www.example.com/path',
    )
    expect(
      compactInsightText('http://example.com/abcdefghijklmnopqrstuvwxyz', 24),
    ).toBe('example.com/abcdefgh…uvwxyz')
    expect(
      compactInsightText(
        'ftp://https://example.com/abcdefghijklmnopqrstuvwxyz',
        24,
      ),
    ).toBe('ftp://https…opqrstuvwxyz')
    expect(compactInsightText('https://%/this-is-long', 16)).toContain('…')
    expect(
      compactInsightText('not-a-url-but-very-very-long-text', 16),
    ).toContain('…')
    expect(compactInsightText('   ', 16)).toBe('')
    expect(hostnameFromUrl('https://www.example.com/path')).toBe('example.com')
    expect(hostnameFromUrl('https://foo.www.example.com/path')).toBe(
      'foo.www.example.com',
    )
    expect(hostnameFromUrl('not a url')).toBeNull()
    expect(hostnameFromUrl(null)).toBeNull()
  })

  test('keeps tiny coverage values honest instead of rounding them to zero', () => {
    expect(formatInsightCoverage(0, 'en')).toBe('0%')
    expect(formatInsightCoverage(0.001, 'en')).toBe('<1%')
    expect(formatInsightCoverage(0.01, 'en')).toBe('1%')
    expect(formatInsightCoverage(0.076, 'en')).toBe('7.6%')
    expect(formatInsightCoverage(0.091, 'en')).toBe('9.1%')
    expect(formatInsightCoverage(0.105, 'en')).toBe('11%')
    expect(formatInsightCoverage(0.76, 'en')).toBe('76%')
  })

  test('recognizes stale runtime mutation races that should refresh instead of erroring', () => {
    expect(
      runtimeJobMutationNeedsRefresh(
        "Intelligence job 56600 is in state 'succeeded' and cannot be cancelled.",
      ),
    ).toBe(true)
    expect(
      runtimeJobMutationNeedsRefresh(
        "Intelligence job 56600 is in state 'running' and cannot be retried.",
      ),
    ).toBe(true)
    expect(runtimeJobMutationNeedsRefresh('429 from upstream host')).toBe(false)
  })
})
