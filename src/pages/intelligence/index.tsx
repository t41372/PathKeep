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
import { TimeRangeSelector } from '../../components/intelligence/time-range-selector'
import { useI18n } from '../../lib/i18n/hooks'
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

  return (
    <div className="intelligence-page" data-testid="intelligence-page">
      <div className="intelligence-page__header">
        <TimeRangeSelector
          key={`${preset}:${dateRange.start}:${dateRange.end}`}
          dateRange={dateRange}
          preset={preset}
          onPresetChange={setPreset}
          onCustomRange={setCustomRange}
          t={t}
        />
        {effectiveProfileId ? (
          <p className="intelligence-page__scope-note">
            {t('scopedViewBody', {
              profile: profileScopeLabel ?? effectiveProfileId,
            })}
          </p>
        ) : null}
      </div>

      <IntelligenceRuntimeDigest
        initialized={Boolean(snapshot?.config.initialized)}
        unlocked={Boolean(snapshot?.archiveStatus.unlocked)}
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
