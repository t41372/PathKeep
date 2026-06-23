/**
 * @file note-codes.ts
 * @description Resolves the backend's stable AI note/warning CODES to localized user-facing copy.
 * @module lib/ai
 *
 * ## Responsibilities
 * - Map each `AiSearchNote` / `AiIndexWarning` / `AiAgentNote` CODE (review-fix M-6/M-7) to its
 *   localized sentence via a namespace translator, weaving any STRUCTURAL interpolation params
 *   (provider id / name) the backend carried so the UI never renders raw English.
 * - Stay exhaustive: every backend variant resolves to a real catalog key, with a closed `switch`
 *   the type checker forces to cover every code (a new variant fails the build until localized).
 *
 * ## Not responsible for
 * - Owning the catalog strings (they live in the `explorer` / `settings` / `assistant` namespaces).
 * - The legacy English `notes` / `warning` strings the backend still carries for the model-facing
 *   agent-tool path + the persisted run trace — those are never shown to the user.
 *
 * ## Dependencies
 * - The intelligence wire types (`AiSearchNote` etc.) and a `(key, vars?) => string` translator the
 *   caller binds to the right namespace.
 *
 * ## Performance notes
 * - Pure, allocation-free dispatch over a tiny closed enum; safe to call inside a render/stream path.
 */

import type {
  AiAgentNote,
  AiIndexWarning,
  AiSearchNote,
  AiSemanticStaleness,
} from '../types/intelligence'

/** A namespace-bound translator: `(key, vars?) => localizedString`. */
export type NoteTranslator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

/**
 * Resolves the localized sentence for one semantic-staleness reason (shared by search + index-health).
 *
 * The catalog keys live wherever the surface's namespace is, so the caller passes the namespace-bound
 * translator AND the key prefix the surface owns; this keeps the shared reason vocabulary localized in
 * one place per surface without hard-coding which namespace it belongs to.
 */
function stalenessKey(reason: AiSemanticStaleness): string {
  switch (reason) {
    case 'watermark':
      return 'StaleWatermark'
    case 'enrichment':
      return 'StaleEnrichment'
  }
}

/**
 * Resolves an AI-search degradation note CODE (review-fix M-6) to localized copy in the `explorer`
 * namespace. The translator must be bound to `explorer`; the catalog keys are `aiSearchNote*`.
 */
export function localizeAiSearchNote(
  note: AiSearchNote,
  t: NoteTranslator,
): string {
  switch (note.code) {
    case 'lexicalFallbackNoProvider':
      return t('aiSearchNoteLexicalFallbackNoProvider')
    case 'emptySemanticIndex':
      return t('aiSearchNoteEmptySemanticIndex')
    case 'semanticMatchesFilteredOut':
      return t('aiSearchNoteSemanticMatchesFilteredOut')
    case 'configDriftDimension':
      return t('aiSearchNoteConfigDriftDimension')
    case 'configDriftFingerprint':
      return t('aiSearchNoteConfigDriftFingerprint')
    case 'stale':
      return t(`aiSearchNote${stalenessKey(note.reason)}`)
    case 'providerResolutionFailed':
      // The opaque transport error is carried structurally; the FE composes the localized sentence.
      return t('aiSearchNoteProviderResolutionFailed', { reason: note.reason })
  }
}

/**
 * Resolves a list of search note codes to localized lines, preserving order. Used by the search route
 * to feed the relevance-notes region with localized copy instead of the raw English `notes`.
 */
export function localizeAiSearchNotes(
  notes: readonly AiSearchNote[] | undefined,
  t: NoteTranslator,
): string[] {
  return (notes ?? []).map((note) => localizeAiSearchNote(note, t))
}

/**
 * Resolves an index-health warning CODE (review-fix M-7) to localized copy in the `settings`
 * namespace. The translator must be bound to `settings`; interpolated variants weave their structural
 * params (provider id / name). `buildFailed` carries the opaque transport reason verbatim (no fixed
 * vocabulary to localize), wrapped in a localized prefix so the surface still reads in-locale.
 */
export function localizeAiIndexWarning(
  warning: AiIndexWarning,
  t: NoteTranslator,
): string {
  switch (warning.code) {
    case 'archiveNotInitialized':
      return t('aiIndexWarningArchiveNotInitialized')
    case 'noEmbeddingProvider':
      return t('aiIndexWarningNoEmbeddingProvider')
    case 'embeddingProviderMissing':
      return t('aiIndexWarningEmbeddingProviderMissing', {
        providerId: warning.providerId,
      })
    case 'embeddingProviderDisabled':
      return t('aiIndexWarningEmbeddingProviderDisabled', {
        providerName: warning.providerName,
      })
    case 'embeddingProviderNoApiKey':
      return t('aiIndexWarningEmbeddingProviderNoApiKey', {
        providerName: warning.providerName,
      })
    case 'embeddingProviderNoModel':
      return t('aiIndexWarningEmbeddingProviderNoModel', {
        providerName: warning.providerName,
      })
    case 'indexNotBuilt':
      return t('aiIndexWarningIndexNotBuilt')
    case 'indexStale':
      return t(`aiIndexWarning${stalenessKey(warning.reason)}`)
    case 'buildFailed':
      return t('aiIndexWarningBuildFailed', { reason: warning.reason })
  }
}

/**
 * Resolves an agent-harness control note CODE (review-fix M-6) to localized copy in the `assistant`
 * namespace. The translator must be bound to `assistant`; the catalog keys are `chatAgentNote*`.
 */
export function localizeAiAgentNote(
  note: AiAgentNote,
  t: NoteTranslator,
): string {
  switch (note.code) {
    case 'maxStepsReached':
      return t('chatAgentNoteMaxStepsReached')
    case 'tokenBudgetReached':
      return t('chatAgentNoteTokenBudgetReached')
    case 'toolCallingUnavailable':
      return t('chatAgentNoteToolCallingUnavailable')
  }
}
