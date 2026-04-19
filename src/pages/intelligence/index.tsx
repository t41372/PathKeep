/**
 * Intelligence route shell for Core Intelligence.
 *
 * Why this file exists:
 * - `/intelligence` is the deterministic analysis route and should focus on route state, scope honesty, and deep-link grammar.
 * - Section rendering lives in sibling modules so this file stays readable when the page grows.
 *
 * Main declarations:
 * - `IntelligencePage`
 *
 * Source-of-truth notes:
 * - Keep this aligned with `docs/features/core-intelligence-ultimate-design.md` §2.2.
 * - Keep route and scope grammar aligned with `docs/design/screens-and-nav.md`.
 */

import './intelligence.css'

import { useShellData } from '../../app/shell-data-context'
import { StatusCallout } from '../../components/primitives/status-callout'
import { TimeRangeSelector } from '../../components/intelligence/time-range-selector'
import { useI18n } from '../../lib/i18n/hooks'
import { Link } from 'react-router-dom'
import { IntelligenceSections } from './sections'
import { useIntelligenceRouteState } from './route-state'
import { IntelligenceRuntimeDigest } from './runtime-digest'
import { intelligenceText } from './copy'

/**
 * Renders the `/intelligence` route.
 */
export function IntelligencePage() {
  const { language, t } = useI18n('intelligence')
  const { snapshot } = useShellData()
  const {
    dateRange,
    effectiveProfileId,
    preset,
    profileScopeLabel,
    setCustomRange,
    setPreset,
    withCurrentRouteSearch,
  } = useIntelligenceRouteState()

  const domainHref = (domain: string) =>
    `/intelligence/domain/${encodeURIComponent(domain)}${withCurrentRouteSearch()}`
  const archiveWideBadge = intelligenceText(language, t, 'archiveWideBadge')
  const archiveWideBody = intelligenceText(language, t, 'archiveWideBody')
  const externalOutputsReviewBody = intelligenceText(
    language,
    t,
    'externalOutputsReviewBody',
  )

  return (
    <div className="intelligence-page" data-testid="intelligence-page">
      <TimeRangeSelector
        key={`${preset}:${dateRange.start}:${dateRange.end}`}
        dateRange={dateRange}
        preset={preset}
        onPresetChange={setPreset}
        onCustomRange={setCustomRange}
        t={t}
      />

      <StatusCallout
        tone="info"
        title={effectiveProfileId ? t('scopedViewTitle') : archiveWideBadge}
        body={
          effectiveProfileId
            ? t('scopedViewBody', {
                profile: profileScopeLabel ?? effectiveProfileId,
              })
            : archiveWideBody
        }
      />

      <IntelligenceRuntimeDigest
        initialized={Boolean(snapshot?.config.initialized)}
        unlocked={Boolean(snapshot?.archiveStatus.unlocked)}
      />

      <StatusCallout
        tone="info"
        title={t('externalOutputsReviewTitle')}
        body={externalOutputsReviewBody}
        actions={
          <Link
            className="btn-secondary"
            to="/settings#settings-external-outputs"
          >
            {t('externalOutputsReviewAction')}
          </Link>
        }
      />

      <IntelligenceSections
        dateRange={dateRange}
        domainHref={domainHref}
        language={language}
        profileId={effectiveProfileId}
        scopeLabel={
          effectiveProfileId
            ? (profileScopeLabel ?? effectiveProfileId)
            : archiveWideBadge
        }
        t={t}
      />
    </div>
  )
}

export { DomainDeepDiveRoutePage } from './domain-deep-dive'
