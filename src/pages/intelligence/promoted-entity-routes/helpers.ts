import {
  localDateKeyFromIso,
  type DateRange,
  type RefindScoreFactor,
  useAsyncData,
} from '../../../lib/core-intelligence'
import * as api from '../../../lib/core-intelligence/api'
import { useI18n } from '../../../lib/i18n/hooks'
import { intelligenceText } from '../copy'
import { useIntelligenceRouteState } from '../route-state'

/**
 * Keeps promoted route scope copy shared between the hook and pure coverage so profile fallback wording cannot drift.
 */
export function buildScopeCalloutCopy({
  archiveWideBadge,
  archiveWideBody,
  effectiveProfileId,
  profileScopeLabel,
  t,
}: {
  archiveWideBadge: string
  archiveWideBody: string
  effectiveProfileId: string | null
  profileScopeLabel: string | null
  t: (key: string, vars?: Record<string, string | number>) => string
}) {
  const scopedProfileLabel = effectiveProfileId
    ? (profileScopeLabel ?? effectiveProfileId)
    : null

  return {
    renderScopeCallout: () => ({
      body: scopedProfileLabel
        ? t('scopedViewBody', {
            profile: scopedProfileLabel,
          })
        : archiveWideBody,
      title: scopedProfileLabel ? t('scopedViewTitle') : archiveWideBadge,
    }),
    scopeLabel: scopedProfileLabel ?? archiveWideBadge,
  }
}

export function useScopeCallout() {
  const { language, t } = useI18n('intelligence')
  const { effectiveProfileId, profileScopeLabel } = useIntelligenceRouteState()
  const archiveWideBadge = intelligenceText(language, t, 'archiveWideBadge')
  const archiveWideBody = intelligenceText(language, t, 'archiveWideBody')
  const scopeCopy = buildScopeCalloutCopy({
    archiveWideBadge,
    archiveWideBody,
    effectiveProfileId,
    profileScopeLabel,
    t,
  })

  return {
    effectiveProfileId,
    profileScopeLabel,
    ...scopeCopy,
  }
}

export function normalizeRefindFactors(value: unknown): RefindScoreFactor[] {
  if (!Array.isArray(value)) {
    return []
  }

  return value.flatMap((entry) => {
    if (!entry || typeof entry !== 'object') {
      return []
    }

    const factor = entry as Record<string, unknown>
    return [
      {
        signal: typeof factor.signal === 'string' ? factor.signal : '',
        rawValue:
          typeof factor.rawValue === 'number' &&
          Number.isFinite(factor.rawValue)
            ? factor.rawValue
            : 0,
        weight:
          typeof factor.weight === 'number' && Number.isFinite(factor.weight)
            ? factor.weight
            : 0,
        contribution:
          typeof factor.contribution === 'number' &&
          Number.isFinite(factor.contribution)
            ? factor.contribution
            : 0,
      },
    ]
  })
}

export function useFocusedCompareSet(
  compareSetId: string | null,
  dateRange: DateRange,
  profileId: string | null,
) {
  return useAsyncData<Awaited<
    ReturnType<typeof api.getCompareSetDetail>
  > | null>(
    () =>
      compareSetId
        ? api.getCompareSetDetail(compareSetId, dateRange, profileId)
        : Promise.resolve(null),
    [compareSetId, dateRange, profileId],
  )
}

export { localDateKeyFromIso }
