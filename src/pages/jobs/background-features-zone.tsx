/**
 * @file background-features-zone.tsx
 * @description Renders 3 status chips for Smart-search index, Site content, and Analysis modules.
 * @module pages/jobs
 *
 * ## Responsibilities
 * - Summarize the state of the three background feature areas in chip form.
 * - Provide a "→ Settings" link from each chip to the relevant settings area.
 *
 * ## Not responsible for
 * - Fetching or polling feature status (caller owns the data).
 * - Rendering job lists or progress bars (see other zone components).
 */

import { Link } from 'react-router-dom'
import type {
  AiIndexStatus,
  IntelligenceRuntimeSnapshot,
} from '../../lib/types'
import type { ResolvedLanguage } from '../../lib/i18n'

interface BackgroundFeaturesZoneProps {
  aiStatus: AiIndexStatus | null
  runtime: IntelligenceRuntimeSnapshot | null
  jobsT: (key: string, vars?: Record<string, string | number>) => string
  language: ResolvedLanguage
}

/**
 * Renders the "Background features" region with 3 summary chips.
 */
export function BackgroundFeaturesZone({
  aiStatus,
  runtime,
  jobsT,
}: BackgroundFeaturesZoneProps) {
  return (
    <section
      className="activity-zone activity-zone--features"
      role="region"
      aria-label={jobsT('backgroundFeaturesTitle')}
    >
      <h2 className="activity-zone__heading">
        {jobsT('backgroundFeaturesTitle')}
      </h2>
      <div className="activity-features-grid">
        <SmartSearchChip aiStatus={aiStatus} jobsT={jobsT} />
        <SiteContentChip runtime={runtime} jobsT={jobsT} />
        <AnalysisChip runtime={runtime} jobsT={jobsT} />
      </div>
    </section>
  )
}

// ── Smart-search chip ─────────────────────────────────────────────────────────

interface SmartSearchChipProps {
  aiStatus: AiIndexStatus | null
  jobsT: (key: string, vars?: Record<string, string | number>) => string
}

function SmartSearchChip({ aiStatus, jobsT }: SmartSearchChipProps) {
  let stateKey: string
  let detail: string

  if (!aiStatus || !aiStatus.enabled) {
    stateKey = 'chipStateOff'
    detail = jobsT('chipSmartSearchOff')
  } else if (
    aiStatus.state === 'building' ||
    aiStatus.queuedJobs > 0 ||
    aiStatus.runningJobs > 0
  ) {
    stateKey = 'chipStateBuilding'
    detail = jobsT('chipSmartSearchBuilding')
  } else if (aiStatus.state === 'ready' && aiStatus.indexedItems > 0) {
    stateKey = 'chipStateReady'
    detail = jobsT('chipSmartSearchIndexed', { count: aiStatus.indexedItems })
  } else if (aiStatus.state === 'degraded') {
    stateKey = 'chipStateDegraded'
    detail = jobsT('chipSmartSearchIndexed', { count: aiStatus.indexedItems })
  } else if (aiStatus.state === 'failed') {
    stateKey = 'chipStateFailed'
    detail = jobsT('chipSmartSearchFailed')
  } else if (aiStatus.indexedItems === 0) {
    stateKey = 'chipStateIdle'
    detail = jobsT('chipSmartSearchEmpty')
  } else {
    stateKey = 'chipStateIdle'
    detail = jobsT('chipSmartSearchEmpty')
  }

  return (
    <div
      className={`feature-chip feature-chip--${stateKey.replace('chipState', '').toLowerCase()}`}
    >
      <div className="feature-chip__header">
        <span className="feature-chip__label">
          {jobsT('chipSmartSearchLabel')}
        </span>
        <span className="feature-chip__state">{jobsT(stateKey)}</span>
      </div>
      <p className="feature-chip__detail">{detail}</p>
      <Link className="feature-chip__settings" to="/settings#ai">
        {jobsT('chipGoToSettings')}
      </Link>
    </div>
  )
}

// ── Site content chip ─────────────────────────────────────────────────────────

interface SiteContentChipProps {
  runtime: IntelligenceRuntimeSnapshot | null
  jobsT: (key: string, vars?: Record<string, string | number>) => string
}

function SiteContentChip({ runtime, jobsT }: SiteContentChipProps) {
  const contentPlugin =
    runtime?.plugins.find((p) => p.pluginId === 'readable-content-refetch') ??
    null

  let stateKey: string
  let detail: string

  if (!contentPlugin) {
    stateKey = 'chipStateOff'
    detail = jobsT('chipSiteContentOff')
  } else if (contentPlugin.queuedJobs > 0) {
    stateKey = 'chipStateIdle'
    detail = jobsT('chipSiteContentQueued', { count: contentPlugin.queuedJobs })
  } else {
    stateKey = 'chipStateReady'
    detail = jobsT('chipSiteContentStored', {
      count: contentPlugin.storedRecords,
    })
  }

  return (
    <div
      className={`feature-chip feature-chip--${stateKey.replace('chipState', '').toLowerCase()}`}
    >
      <div className="feature-chip__header">
        <span className="feature-chip__label">
          {jobsT('chipSiteContentLabel')}
        </span>
        <span className="feature-chip__state">{jobsT(stateKey)}</span>
      </div>
      <p className="feature-chip__detail">{detail}</p>
      <Link className="feature-chip__settings" to="/settings">
        {jobsT('chipGoToSettings')}
      </Link>
    </div>
  )
}

// ── Analysis chip ─────────────────────────────────────────────────────────────

interface AnalysisChipProps {
  runtime: IntelligenceRuntimeSnapshot | null
  jobsT: (key: string, vars?: Record<string, string | number>) => string
}

function AnalysisChip({ runtime, jobsT }: AnalysisChipProps) {
  const attentionCount =
    runtime?.modules.filter((m) => m.status !== 'ready').length ?? 0

  let stateKey: string
  let detail: string

  if (attentionCount > 0) {
    stateKey = 'chipStateDegraded'
    detail = jobsT('chipAnalysisAttention', { count: attentionCount })
  } else {
    stateKey = 'chipStateReady'
    detail = jobsT('chipAnalysisReady')
  }

  return (
    <div
      className={`feature-chip feature-chip--${stateKey.replace('chipState', '').toLowerCase()}`}
    >
      <div className="feature-chip__header">
        <span className="feature-chip__label">
          {jobsT('chipAnalysisLabel')}
        </span>
        <span className="feature-chip__state">{jobsT(stateKey)}</span>
      </div>
      <p className="feature-chip__detail">{detail}</p>
      <Link className="feature-chip__settings" to="/settings">
        {jobsT('chipGoToSettings')}
      </Link>
    </div>
  )
}
