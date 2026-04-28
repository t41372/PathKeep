/**
 * This test file protects the compact build-label formatting contract.
 */

import { describe, expect, test } from 'vitest'
import {
  formatBuildRevisionLabel,
  formatBuildVersionLabel,
  formatBuildVersionTitle,
} from './build-info'

const baseBuildInfo = {
  version: '0.1.0',
  gitCommitShort: 'abc1234',
  gitCommitFull: 'abc1234def5678',
  gitDirty: false,
} as const

describe('build-info formatting', () => {
  test('formats a compact version label with the short revision', () => {
    expect(formatBuildVersionLabel(baseBuildInfo)).toBe('v0.1.0 · abc1234')
  })

  test('adds a plus suffix when the build was compiled from a dirty worktree', () => {
    expect(
      formatBuildRevisionLabel({
        ...baseBuildInfo,
        gitDirty: true,
      }),
    ).toBe('abc1234+')
    expect(
      formatBuildVersionLabel({
        ...baseBuildInfo,
        gitDirty: true,
      }),
    ).toBe('v0.1.0 · abc1234+')
  })

  test('hides unknown revisions instead of inventing a fake hash', () => {
    expect(
      formatBuildRevisionLabel({
        ...baseBuildInfo,
        gitCommitShort: 'unknown',
        gitCommitFull: 'unknown',
      }),
    ).toBeNull()
    expect(
      formatBuildVersionLabel({
        ...baseBuildInfo,
        gitCommitShort: 'unknown',
        gitCommitFull: 'unknown',
      }),
    ).toBe('v0.1.0')
  })

  test('formats a tooltip title with the full revision when available', () => {
    expect(formatBuildVersionTitle(baseBuildInfo)).toBe(
      '0.1.0 (abc1234def5678)',
    )
    expect(
      formatBuildVersionTitle({
        ...baseBuildInfo,
        gitDirty: true,
      }),
    ).toBe('0.1.0 (abc1234def5678+)')
    expect(
      formatBuildVersionTitle({
        ...baseBuildInfo,
        gitCommitShort: 'unknown',
        gitCommitFull: 'unknown',
      }),
    ).toBe('0.1.0')
  })

  test('trims revision metadata and refuses unknown full hashes in titles', () => {
    expect(
      formatBuildRevisionLabel({
        ...baseBuildInfo,
        gitCommitShort: '  abc1234  ',
      }),
    ).toBe('abc1234')
    expect(
      formatBuildVersionTitle({
        ...baseBuildInfo,
        gitCommitShort: 'abc1234',
        gitCommitFull: '  abc1234def5678  ',
      }),
    ).toBe('0.1.0 (abc1234def5678)')
    expect(
      formatBuildVersionTitle({
        ...baseBuildInfo,
        gitCommitShort: 'abc1234',
        gitCommitFull: 'unknown',
      }),
    ).toBe('0.1.0')
    expect(
      formatBuildVersionTitle({
        ...baseBuildInfo,
        gitCommitShort: 'abc1234',
        gitCommitFull: '   ',
      }),
    ).toBe('0.1.0')
  })
})
