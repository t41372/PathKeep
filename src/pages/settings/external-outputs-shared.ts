/**
 * @file external-outputs-shared.ts
 * @description Shares the stable types and pure helper logic behind the Settings external-output review surface.
 * @module pages/settings
 *
 * ## Responsibilities
 * - Define the small shared contracts that the external-output tabs reuse.
 * - Hold pure formatting helpers so tab components stay render-focused.
 *
 * ## Not responsible for
 * - Fetching external-output data from Core Intelligence
 * - Rendering tab-specific review UI
 *
 * ## Dependencies
 * - Depends on Core Intelligence output and overview types only.
 *
 * ## Performance notes
 * - Keep helpers pure and allocation-light because widget and public tabs rebuild their metric rows on each render.
 */

import type {
  DigestSummary,
  IntelligenceEmbedCardPayload,
  IntelligencePublicSnapshot,
  IntelligenceWidgetSnapshot,
} from '../../lib/core-intelligence'

/**
 * Narrows the manual external-output surface to the three review tabs that Settings actually ships.
 */
export type OutputTab = 'embed' | 'widget' | 'public'

/**
 * Documents the route-local translator shape shared by the split tab renderers.
 */
export type Translate = (
  key: string,
  vars?: Record<string, string | number>,
) => string

/**
 * Groups the three external-output payloads that Settings loads together from Core Intelligence.
 */
export interface ExternalOutputsPayload {
  embedCards: IntelligenceEmbedCardPayload[]
  widgetSnapshot: IntelligenceWidgetSnapshot
  publicSnapshot: IntelligencePublicSnapshot
}

/**
 * Turns one external-output payload into deterministic review JSON so manual copy/export stays stable.
 */
export function prettyJson(value: unknown) {
  return JSON.stringify(value, null, 2)
}

/**
 * Converts the shared digest summary into the metric-grid items shown in widget and public previews.
 *
 * This exists to keep the repeated metric formatting logic in one place while preserving locale-aware
 * number formatting and the existing metric order that the Settings review surface already shipped.
 */
export function buildDigestMetricItems(
  digestSummary: DigestSummary,
  language: string,
  intelligenceT: Translate,
) {
  return [
    {
      label: intelligenceT('digestVisits'),
      value: digestSummary.totalVisits.value.toLocaleString(language),
    },
    {
      label: intelligenceT('digestSearches'),
      value: digestSummary.totalSearches.value.toLocaleString(language),
    },
    {
      label: intelligenceT('digestNewSites'),
      value: digestSummary.newDomains.value.toLocaleString(language),
    },
    {
      label: intelligenceT('digestDeepRead'),
      value: digestSummary.deepReadPages.value.toLocaleString(language),
    },
    {
      label: intelligenceT('digestRefind'),
      value: digestSummary.refindPages.value.toLocaleString(language),
    },
  ]
}

/**
 * Converts known backend-authored card titles into locale-owned UI copy while
 * leaving user/domain data and unknown future card titles untouched.
 */
export function localizeOutputCardTitle(title: string, t: Translate): string {
  const onThisDayMatch = title.match(/^On This Day · (\d{4})$/)
  if (onThisDayMatch) {
    return t('externalOutputsCardOnThisDayTitle', {
      year: onThisDayMatch[1],
    })
  }

  switch (title) {
    case 'Visits':
      return t('externalOutputsCardVisitsTitle')
    case 'Searches':
      return t('externalOutputsCardSearchesTitle')
    default:
      return title
  }
}

/**
 * Converts known backend-authored badge copy into locale-owned UI copy.
 */
export function localizeOutputCardEyebrow(
  eyebrow: string | null,
  t: Translate,
): string | null {
  switch (eyebrow?.trim().toUpperCase()) {
    case 'TOP SITE':
      return t('externalOutputsCardTopSiteEyebrow')
    case 'REFIND':
      return t('externalOutputsCardRefindEyebrow')
    case 'STABLE SOURCE':
      return t('externalOutputsCardStableSourceEyebrow')
    default:
      return eyebrow
  }
}

/**
 * Converts known backend-authored card bodies into locale-owned UI copy so
 * integrations can show a human preview before exposing raw payloads.
 */
export function localizeOutputCardBody(body: string, t: Translate): string {
  if (body === 'Total visits in the selected intelligence window.') {
    return t('externalOutputsCardTotalVisitsBody')
  }
  if (
    body === 'Total search events observed in the selected intelligence window.'
  ) {
    return t('externalOutputsCardTotalSearchesBody')
  }

  const topDomainMatch = body.match(
    /^(.+) was one of the most frequently visited domains in this window\.$/,
  )
  if (topDomainMatch) {
    return t('externalOutputsCardTopDomainBody', {
      domain: topDomainMatch[1],
    })
  }

  const refindMatch = body.match(
    /^This page kept resurfacing across ([0-9]+) days and ([0-9]+) trails\.$/,
  )
  if (refindMatch) {
    return t('externalOutputsCardRefindBody', {
      days: refindMatch[1],
      trails: refindMatch[2],
    })
  }

  const sourceMatch = body.match(
    /^(.+) often resolves trails as a (.+) source\.$/,
  )
  if (sourceMatch) {
    const source =
      sourceMatch[2] === 'reference'
        ? t('externalOutputsCardSourceReference')
        : sourceMatch[2]

    return t('externalOutputsCardSourceBody', {
      domain: sourceMatch[1],
      source,
    })
  }

  const browsingMatch = body.match(/^Mostly browsing (.+)$/)
  if (browsingMatch) {
    return t('externalOutputsCardMostlyBrowsingBody', {
      domain: browsingMatch[1],
    })
  }

  return body
}
