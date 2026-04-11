/**
 * This module extracts the pure data helpers that keep the Settings route from turning every small transformation into inline page noise.
 *
 * Why this file exists:
 * - These helpers are easy to unit-test in isolation and make the giant Settings route read more like workflow code than like a pile of micro-utilities.
 * - The functions here mostly deal with cloning, comparison, icon labeling, and retention-selection defaults that multiple Settings actions rely on.
 *
 * Main declarations:
 * - `cloneAiProviderConfig`
 * - `cloneAiSettings`
 * - `serializeAiSettings`
 * - `buildRetentionSelection`
 * - `browserIcon`
 * - `browserIconClass`
 *
 * Source-of-truth notes:
 * - Settings is the control tower route described in `docs/design/screens-and-nav.md`, so even small helpers should preserve review-first, explicit behavior.
 * - Keeping these helpers pure makes it easier to prove that draft state and retention selection rules behave the way the UI copy promises.
 */

import type {
  AiProviderConfig,
  AiProviderPurpose,
  AiSettings,
  ScheduleStatus,
  SecurityStatus,
  RetentionPreview,
} from '../../lib/types'

/**
 * Creates a defensive copy of ai provider config.
 *
 * This helper is intentionally pure so the Settings route can reuse it without dragging more hidden behavior into an already large page component.
 */
export function cloneAiProviderConfig(
  provider: AiProviderConfig,
): AiProviderConfig {
  return {
    ...provider,
    modelCatalog: [...provider.modelCatalog],
  }
}

/**
 * Describes the small support snapshot that Settings reads from the schedule
 * and security review surfaces.
 */
export interface SupportState {
  scheduleStatus: ScheduleStatus | null
  securityStatus: SecurityStatus | null
}

/**
 * Creates a defensive copy of ai settings.
 *
 * This helper is intentionally pure so the Settings route can reuse it without dragging more hidden behavior into an already large page component.
 */
export function cloneAiSettings(settings: AiSettings): AiSettings {
  return {
    ...settings,
    llmProviders: settings.llmProviders.map(cloneAiProviderConfig),
    embeddingProviders: settings.embeddingProviders.map(cloneAiProviderConfig),
  }
}

/**
 * Serializes ai settings so the UI can compare it safely.
 *
 * This helper is intentionally pure so the Settings route can reuse it without dragging more hidden behavior into an already large page component.
 */
export function serializeAiSettings(settings: AiSettings | null | undefined) {
  return settings ? JSON.stringify(settings) : null
}

/**
 * Updates one provider list by ID and returns the new list.
 */
function patchProviderList(
  providers: AiProviderConfig[],
  providerId: string,
  updater: (provider: AiProviderConfig) => AiProviderConfig,
) {
  return providers.map((provider) =>
    provider.id === providerId ? updater(provider) : provider,
  )
}

/**
 * Mirrors secret-storage changes back into the working AI settings draft.
 */
export function mergeAiProviderSecretState(
  settings: AiSettings,
  providerId: string,
  apiKeySaved: boolean,
) {
  return {
    ...settings,
    llmProviders: patchProviderList(
      settings.llmProviders,
      providerId,
      (provider) => ({
        ...provider,
        apiKeySaved,
      }),
    ),
    embeddingProviders: patchProviderList(
      settings.embeddingProviders,
      providerId,
      (provider) => ({
        ...provider,
        apiKeySaved,
      }),
    ),
  }
}

/**
 * Appends a new provider draft to the requested provider list.
 */
export function appendAiProviderDraft(
  settings: AiSettings,
  purpose: AiProviderPurpose,
  provider: AiProviderConfig,
) {
  return purpose === 'llm'
    ? {
        ...settings,
        llmProviders: [...settings.llmProviders, provider],
      }
    : {
        ...settings,
        embeddingProviders: [...settings.embeddingProviders, provider],
      }
}

/**
 * Applies a partial patch to one provider draft while leaving unrelated
 * providers untouched.
 */
export function patchAiProviderDraft(
  settings: AiSettings,
  purpose: AiProviderPurpose,
  providerId: string,
  patch: Partial<AiProviderConfig>,
) {
  return purpose === 'llm'
    ? {
        ...settings,
        llmProviders: patchProviderList(
          settings.llmProviders,
          providerId,
          (provider) => ({
            ...provider,
            ...patch,
          }),
        ),
      }
    : {
        ...settings,
        embeddingProviders: patchProviderList(
          settings.embeddingProviders,
          providerId,
          (provider) => ({
            ...provider,
            ...patch,
          }),
        ),
      }
}

/**
 * Removes one provider draft and clears the selected provider ID if the
 * removed draft was active.
 */
export function removeAiProviderDraft(
  settings: AiSettings,
  purpose: AiProviderPurpose,
  providerId: string,
) {
  if (purpose === 'llm') {
    const llmProviders = settings.llmProviders.filter(
      (provider) => provider.id !== providerId,
    )
    return {
      ...settings,
      llmProviders,
      llmProviderId:
        settings.llmProviderId === providerId ? null : settings.llmProviderId,
    }
  }

  const embeddingProviders = settings.embeddingProviders.filter(
    (provider) => provider.id !== providerId,
  )
  return {
    ...settings,
    embeddingProviders,
    embeddingProviderId:
      settings.embeddingProviderId === providerId
        ? null
        : settings.embeddingProviderId,
  }
}

/**
 * Marks one provider draft as selected for the requested purpose.
 */
export function selectAiProviderDraft(
  settings: AiSettings,
  purpose: AiProviderPurpose,
  providerId: string,
) {
  return purpose === 'llm'
    ? {
        ...settings,
        llmProviderId: providerId,
      }
    : {
        ...settings,
        embeddingProviderId: providerId,
      }
}

/**
 * Builds the initial retention selection state for the current preview.
 *
 * Existing user choices win; newly discovered buckets default from whether
 * they actually contain bytes so the UI does not silently auto-select empty
 * buckets.
 */
export function buildRetentionSelection(
  preview: RetentionPreview,
  current: Record<string, boolean> = {},
) {
  return Object.fromEntries(
    preview.buckets.map((bucket) => [
      bucket.id,
      current[bucket.id] ?? bucket.bytes > 0,
    ]),
  )
}

/**
 * Resolves the small browser glyph treatment used by the Settings profile review list.
 *
 * This helper is intentionally pure so the Settings route can reuse it without dragging more hidden behavior into an already large page component.
 */
export function browserIcon(profileId: string): string {
  const kind = profileId.split(':')[0]
  if (kind === 'chrome') return 'C'
  if (kind === 'arc') return 'A'
  if (kind === 'firefox') return 'F'
  if (kind === 'safari') return 'S'
  return kind[0]?.toUpperCase() ?? '?'
}

/**
 * Resolves the small browser glyph treatment used by the Settings profile review list.
 *
 * This helper is intentionally pure so the Settings route can reuse it without dragging more hidden behavior into an already large page component.
 */
export function browserIconClass(profileId: string): string {
  const kind = profileId.split(':')[0]
  return `browser-icon ${kind}`
}
