import { describe, expect, test } from 'vitest'
import {
  compactInsightText,
  formatInsightCoverage,
  runtimeJobMutationNeedsRefresh,
  summarizeRuntimeJobError,
} from './intelligence-presentation'

const jobsT = (key: string) => {
  switch (key) {
    case 'noErrorDetails':
      return 'No details yet.'
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
          lastError: '429 from upstream host',
          url: 'https://example.com/article',
        },
        jobsT,
      ),
    ).toBe('The upstream site temporarily refused the fetch. Retry it later.')
  })

  test('compacts long tracking-style URLs into readable labels', () => {
    const rawUrl =
      'https://elink2fb.mail.gethalfbaked.com/ss/c/u001.W7yY1DE5FLIFsRdzd8xlOFxIFmg-LnZyrakPeT0Kr4Akcsd_1nNyG4O-JzeKCfzCHx0L-Q-XaEfbRodxc4QPmYWsoxwYVKVdTuQECM3bSYEh-a_vxV99Ks-5wLaLiMaeY37qxnfpzNzmpXsxTvq_IPXz5HgP5iJZKdDkWjrUqimIY5PctDJFjhBu_zPRRgAg-dQNhu6BYaOyTuNS1qwtzKJBmoFK-4zVhhNl9iu13mgWvQtpcozXWzWK8THCZHcPxnrCdMIIBGV828w4PzF5yB_x3RR-pCoiYJSxvPV7JY5xBOnmLvfnv6r5NzjB43ogYyVW_4TBHTEm9p1YBYuXct8p7_gVGKhCalmuEO4l29RrpEepk_Zt8ZfJbgCHA2NjhaShq8P_ecCqoZkhZbPdK7hXCmS6vCQQRwbCqJMn7rA_6CzHp8cuu2dLM1WxsvcV1C5v3062OGqpliWy3p_B6ZqbcyyyAVhk3tEUvBYh8TUYjsNVDIOCyEAc0dkLDNoDaBX2UtwqDtlcFjtIHf7x8fmFqODD0nlBa94zHuY8gUv1PcmjeDQU_ibOvtlZSJutjF08QvPetF_tT9R1AG-pCUZYPqZPxqcyIhSnpMPA85mgUOD4Ssj59-ReWgfWvpXDFwIpILbfYzvkr6rd2QwWYR7mD2nmsRHmhGPLM4ar_Tdo8VRzL00zEoB-LSpWB_aZAa1FIBk3dWFYNrjwOwn-R6p4vxsbWIzCgtsPQEIjA8CHYwMpgFjDUz7hRPmSHIzKIE4t5jS9G3uUkMWr1m-_JqtApppz5izeYwpp8sYQOhIp02VjCNahZjKvFVAel4258O1sayVYHxyrT181U82dM1cobtRdxHUToaSFPC_2voJT-pw2NzWLA8-sy0gVhOtHAn0VGS_TSTRrJWoukJubqwM2cjTB7VvMhREDJND84eG0Lz0CeZue7gVy4dz-O1UMAy6_ghvYEslOJ8MYK71du2uV8w8d26maPf_m_b6HuuOddElBfgZjYFWwhpjNModDmZKXHamsBFP3Xt-6an2SyB51igaGpmn190V5Zvo8kwk/4po/kXT0fItVSwCLj4QagxK9mw/h35/h001.Asq1GllC9cmWzWIRdZ3K50d8fC8sPF_cT-N0wkOUle8'

    const compact = compactInsightText(rawUrl, 88)
    expect(compact).toContain('elink2fb.mail.gethalfbaked.com')
    expect(compact).toContain('…')
    expect(compact.length).toBeLessThanOrEqual(89)
  })

  test('keeps tiny coverage values honest instead of rounding them to zero', () => {
    expect(formatInsightCoverage(0.001, 'en')).toBe('<1%')
    expect(formatInsightCoverage(0.076, 'en')).toBe('7.6%')
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
    expect(runtimeJobMutationNeedsRefresh('429 from upstream host')).toBe(
      false,
    )
  })
})
