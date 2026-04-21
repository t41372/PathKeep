/**
 * @file types-outputs.ts
 * @description Deterministic share/export/local-host payload types built from Core Intelligence aggregates.
 * @module core-intelligence/types
 *
 * ## Responsibilities
 * - Own embed-card, widget snapshot, public snapshot, and local-host artifact contracts.
 * - Keep trusted-output payload shapes separate from analysis read models.
 *
 * ## Not responsible for
 * - Owning the aggregate metrics and entity lists that feed these payloads.
 * - Defining navigation/session/trail analysis types.
 *
 * ## Dependencies
 * - Depends on shared primitives plus overview and analysis payload types.
 * - Consumed by settings external-output surfaces and local-host preview/build commands.
 *
 * ## Performance notes
 * - Type-only module; isolating output payloads reduces churn in route-facing type files when share/export contracts evolve.
 */

import type { DateRange, InsightEntityReference } from './types-primitives'
import type { DiscoveryTrend } from './types-analysis'
import type { DigestSummary, EngineRanking } from './types-overview'

/** Shareable/embed-oriented card payload from backend-only provider commands. */
export interface IntelligenceEmbedCardPayload {
  cardId: string
  cardType: string
  title: string
  eyebrow?: string | null
  body: string
  metricLabel?: string | null
  metricValue?: string | null
  href?: string | null
  primaryTarget?: InsightEntityReference | null
  secondaryTargets?: InsightEntityReference[]
  internalOnly: boolean
}

/** Compact widget snapshot built from aggregate Core Intelligence read models. */
export interface IntelligenceWidgetSnapshot {
  generatedAt: string
  dateRange: DateRange
  digestSummary: DigestSummary
  highlights: IntelligenceEmbedCardPayload[]
  notes: string[]
}

/** Redacted public snapshot that intentionally omits visit-level drilldown fields. */
export interface IntelligencePublicSnapshot {
  generatedAt: string
  dateRange: DateRange
  digestSummary: DigestSummary
  topDomains: string[]
  searchEngines: EngineRanking[]
  discoveryTrend: DiscoveryTrend
  notes: string[]
}

/** One generated file belonging to a reusable local host artifact. */
export interface IntelligenceLocalHostGeneratedFile {
  relativePath: string
  absolutePath?: string | null
  purpose: string
  contents: string
}

/** Request payload for deterministic local-host preview/build commands. */
export interface IntelligenceLocalHostRequest {
  dateRange: DateRange
  profileId?: string | null
  locale: string
}

/** Machine-readable bundle persisted beside one local host artifact. */
export interface IntelligenceLocalHostBundle {
  bundleVersion: string
  hostId: string
  generatedAt: string
  locale: string
  dateRange: DateRange
  profileId?: string | null
  embedCards: IntelligenceEmbedCardPayload[]
  widgetSnapshot: IntelligenceWidgetSnapshot
  publicSnapshot: IntelligencePublicSnapshot
  trustedOnlyCardIds: string[]
  trustedOnlyCardCount: number
  boundaryNotes: string[]
}

/** Existing installed local host discovered on disk for verify UI. */
export interface IntelligenceInstalledLocalHost {
  artifactRoot: string
  entryFilePath: string
  bundle: IntelligenceLocalHostBundle
}

/** Preview payload for one deterministic local host without writing files yet. */
export interface IntelligenceLocalHostPreview {
  artifactRoot: string
  entryFilePath: string
  generatedFiles: IntelligenceLocalHostGeneratedFile[]
  bundle: IntelligenceLocalHostBundle
  boundaryNotes: string[]
  manualSteps: string[]
  warnings: string[]
  installedHost?: IntelligenceInstalledLocalHost | null
}

/** Result payload after writing one deterministic local host artifact. */
export interface IntelligenceLocalHostBuildResult {
  artifactRoot: string
  entryFilePath: string
  generatedFiles: IntelligenceLocalHostGeneratedFile[]
  bundle: IntelligenceLocalHostBundle
  boundaryNotes: string[]
  manualSteps: string[]
  warnings: string[]
  installedHost?: IntelligenceInstalledLocalHost | null
}
