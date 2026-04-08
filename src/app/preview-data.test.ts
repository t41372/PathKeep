import { describe, expect, test } from 'vitest'
import {
  dashboardStats,
  insightHighlights,
  onboardingSteps,
  queueItems,
  recentRuns,
  shellStatus,
} from './preview-data'

describe('preview data fixtures', () => {
  test('keeps the shell fixtures internally consistent', () => {
    expect(shellStatus.archiveHealth).toBeTruthy()
    expect(dashboardStats).toHaveLength(4)
    expect(recentRuns.every((run) => run.status === 'COMPLETED')).toBe(true)
    expect(queueItems.map((item) => item.state)).toEqual([
      'running',
      'ready',
      'blocked',
    ])
    expect(insightHighlights[0]?.title).toContain('Gaussian')
    expect(onboardingSteps.map((step) => step.title)).toEqual([
      'Preview',
      'Manual',
      'Execute',
    ])
  })
})
