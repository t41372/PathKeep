/**
 * This module formats the compact build diagnostics shown in the desktop shell.
 *
 * Why this file exists:
 * - Multiple surfaces show the same version/build metadata, so the formatting
 *   contract should live in one place instead of drifting across routes.
 * - Support and QA need a compact label that still exposes the short commit
 *   and dirty-worktree hint without making every component reimplement it.
 *
 * Main declarations:
 * - `formatBuildRevisionLabel`
 * - `formatBuildVersionLabel`
 *
 * Source-of-truth notes:
 * - The raw metadata comes from `app_build_info`; this helper only shapes how
 *   the compact UI surfaces present that information.
 * - Keep unknown/preview fallbacks readable instead of showing fake hashes.
 */

import type { AppBuildInfo } from './types'

type BuildInfoSummary = Pick<
  AppBuildInfo,
  'version' | 'gitCommitShort' | 'gitCommitFull' | 'gitDirty'
>

/**
 * Returns the short revision label used in compact chrome surfaces.
 */
export function formatBuildRevisionLabel(
  buildInfo: BuildInfoSummary | null | undefined,
) {
  if (!buildInfo) {
    return null
  }

  const short = buildInfo.gitCommitShort.trim()
  if (!short || short === 'unknown') {
    return null
  }

  return `${short}${buildInfo.gitDirty ? '+' : ''}`
}

/**
 * Returns the compact version label used beside the product name.
 */
export function formatBuildVersionLabel(
  buildInfo: BuildInfoSummary | null | undefined,
) {
  if (!buildInfo) {
    return null
  }

  const revision = formatBuildRevisionLabel(buildInfo)
  return revision
    ? `v${buildInfo.version} · ${revision}`
    : `v${buildInfo.version}`
}

/**
 * Returns the hover title used for the compact version badge.
 */
export function formatBuildVersionTitle(
  buildInfo: BuildInfoSummary | null | undefined,
) {
  if (!buildInfo) {
    return null
  }

  const revision = formatBuildRevisionLabel(buildInfo)
  const full = buildInfo.gitCommitFull.trim()

  if (revision && full && full !== 'unknown') {
    return `${buildInfo.version} (${full}${buildInfo.gitDirty ? '+' : ''})`
  }

  return buildInfo.version
}
