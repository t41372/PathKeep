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
