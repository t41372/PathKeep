import { describe, expect, test, vi } from 'vitest'
import {
  dueAfterOptions,
  onboardingStepKeys,
  schedulePlatformLabel,
} from './shared'

describe('onboarding shared helpers', () => {
  test('exports the stable onboarding step and backup interval contracts', () => {
    expect(onboardingStepKeys).toEqual([
      'stepWelcome',
      'stepBrowsers',
      'stepStorage',
      'stepSecurity',
      'stepSchedule',
      'stepAi',
      'stepReady',
    ])
    expect(dueAfterOptions).toEqual([6, 12, 24, 72])
  })

  test('localizes known scheduler platforms and preserves unknown platform ids', () => {
    const t = vi.fn((key: string) => `translated:${key}`)

    expect(schedulePlatformLabel('macos', t)).toBe(
      'translated:platform.macosLabel',
    )
    expect(schedulePlatformLabel('windows', t)).toBe(
      'translated:platform.windowsLabel',
    )
    expect(schedulePlatformLabel('linux', t)).toBe(
      'translated:platform.linuxLabel',
    )
    expect(schedulePlatformLabel('freebsd', t)).toBe('freebsd')
  })
})
