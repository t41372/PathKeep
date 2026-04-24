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
  AiIntegrationPreview,
  AiProviderPurpose,
  AiRequestFormat,
  AiSettings,
  ScheduleStatus,
  SecurityStatus,
  RetentionPreview,
} from '../../lib/types'
import type {
  IntelligenceLocalHostPreview,
  SearchEngineRule,
  SearchEngineRuleInput,
} from '../../lib/core-intelligence/types'

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

type SettingsTranslator = (
  key: string,
  vars?: Record<string, string | number>,
) => string

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
 * Creates the editable draft shape used by the search-engine rule editor.
 *
 * This keeps built-in and custom rule rows aligned with the same draft model,
 * so the Settings route does not need to duplicate empty-state wiring and
 * "edit existing rule" normalization inline.
 */
export function buildSearchEngineRuleDraft(
  rule?: SearchEngineRule | null,
): SearchEngineRuleInput {
  return {
    ruleId: rule?.ruleId ?? null,
    engineId: rule?.engineId ?? '',
    displayName: rule?.displayName ?? '',
    hostPattern: rule?.hostPattern ?? '',
    pathPrefix: rule?.pathPrefix ?? '',
    queryParamKey: rule?.queryParamKey ?? '',
    enabled: rule?.enabled ?? true,
    note: rule?.note ?? '',
    exampleUrl: rule?.exampleUrl ?? '',
  }
}

/**
 * Normalizes the search-engine rule draft before it crosses the backend
 * boundary.
 *
 * The editor is intentionally forgiving while people type, but persistence
 * should collapse empty strings back to null for optional fields so rebuild
 * payloads stay stable and review output stays honest.
 */
export function normalizeSearchEngineRuleDraft(
  draft: SearchEngineRuleInput,
): SearchEngineRuleInput {
  return {
    ruleId: draft.ruleId?.trim() || null,
    engineId: draft.engineId.trim(),
    displayName: draft.displayName.trim(),
    hostPattern: draft.hostPattern.trim(),
    pathPrefix: draft.pathPrefix?.trim() || null,
    queryParamKey: draft.queryParamKey.trim(),
    enabled: draft.enabled,
    note: draft.note?.trim() || null,
    exampleUrl: draft.exampleUrl?.trim() || null,
  }
}

/**
 * Seeds a new AI provider draft with format-specific defaults.
 *
 * The Settings UI lets people add providers before they know every field, so
 * the draft starts with a truthful localhost/cloud preset instead of an empty
 * blob that would force every route slice to reinvent the same defaults.
 */
export function makeDefaultAiProviderDraft(
  purpose: AiProviderPurpose,
  format: AiRequestFormat,
): AiProviderConfig {
  const presets: Record<
    AiRequestFormat,
    { name: string; baseUrl: string; model: string; embModel: string }
  > = {
    ollama: {
      name: 'Ollama',
      baseUrl: 'http://localhost:11434',
      model: 'llama3.2:8b',
      embModel: 'nomic-embed-text',
    },
    'lm-studio': {
      name: 'LM Studio',
      baseUrl: 'http://localhost:1234/v1',
      model: 'local-model',
      embModel: 'local-embed',
    },
    openai: {
      name: 'OpenAI',
      baseUrl: 'https://api.openai.com/v1',
      model: 'gpt-4o',
      embModel: 'text-embedding-3-small',
    },
    anthropic: {
      name: 'Anthropic',
      baseUrl: 'https://api.anthropic.com',
      model: 'claude-sonnet-4-6',
      embModel: 'voyage-3',
    },
    google: {
      name: 'Google',
      baseUrl: 'https://generativelanguage.googleapis.com',
      model: 'gemini-2.0-flash',
      embModel: 'text-embedding-004',
    },
  }
  const preset = presets[format]
  return {
    id: `${format}-${purpose}-${Date.now()}`,
    name: preset.name,
    purpose,
    requestFormat: format,
    enabled: true,
    baseUrl: preset.baseUrl,
    apiKeySaved: false,
    defaultModel: purpose === 'llm' ? preset.model : preset.embModel,
    modelCatalog: [],
    temperature: purpose === 'llm' ? 0.7 : null,
    maxTokens: purpose === 'llm' ? 1200 : null,
    dimensions: purpose === 'embedding' ? 1536 : null,
    notes: null,
  }
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

/**
 * Localizes AI integration preview sentences so Settings never ships the raw
 * English backend copy as its final UI.
 */
function localizeAiIntegrationLine(
  value: string,
  t: SettingsTranslator,
): string {
  switch (value) {
    case 'External AI integrations stay local-first and explicit. PathKeep only exposes localhost MCP tools after you turn on AI + MCP in Settings, and the current app session must stay unlocked.':
    case 'External AI integrations stay local-first and only start after the user enables them in Settings.':
      return t('aiIntegrationConsentSummary')
    case 'Enable MCP or Skill integration in Settings first. Both are off by default.':
    case 'Enable MCP or Skill integration in Settings first.':
      return t('aiIntegrationManualEnable')
    case 'Store the database key in the native keyring if the archive is encrypted, so background and MCP lookups can unlock the archive.':
    case 'Store the database key in the native keyring if the archive is encrypted.':
      return t('aiIntegrationManualStoreKey')
    case 'Copy the generated MCP JSON into your local MCP client configuration and restart that client.':
    case 'Copy the generated MCP JSON into your MCP client configuration.':
      return t('aiIntegrationManualCopyJson')
    case 'Copy the generated skill markdown into your local skills directory if you want a reusable history-research workflow.':
      return t('aiIntegrationManualCopySkill')
    case 'MCP server toggle is currently enabled in saved Settings.':
      return t('aiIntegrationCapabilityMcpEnabled')
    case 'MCP server toggle is currently disabled in saved Settings.':
      return t('aiIntegrationCapabilityMcpDisabled')
    case 'Skill integration toggle is currently enabled in saved Settings.':
      return t('aiIntegrationCapabilitySkillEnabled')
    case 'Skill integration toggle is currently disabled in saved Settings.':
      return t('aiIntegrationCapabilitySkillDisabled')
    case 'Semantic retrieval can use the configured embedding provider when the semantic index is built.':
      return t('aiIntegrationCapabilityEmbeddingEnabled')
    case 'No embedding provider is selected right now, so MCP and external assistants fall back to lexical recall only. They still respect archive visibility and App Lock.':
    case 'No embedding provider is selected right now, so external tools fall back to lexical recall only.':
      return t('aiIntegrationCapabilityEmbeddingDisabled')
    case 'Queries only see currently visible archive facts. Reverted visits stay hidden even if an old embedding row still exists.':
    case 'Only visible archive facts are returned to external tools.':
      return t('aiIntegrationScopeVisibleOnly')
    case 'If App Lock re-locks the session, MCP search returns a locked refusal instead of reading the archive behind the UI.':
    case 'If App Lock re-locks the session, MCP search returns a locked refusal.':
      return t('aiIntegrationScopeLock')
    case 'The MCP surface is localhost-only and never publishes the archive to a remote PathKeep service.':
      return t('aiIntegrationScopeLocalhost')
    case 'Every MCP request is recorded as a dedicated `mcp_query` run in the unified archive ledger.':
    case 'Each MCP search writes a dedicated run-ledger entry.':
      return t('aiIntegrationAuditMcp')
    case 'Assistant answers keep their provider snapshot, retrieval provider, and citations inside `ai_assistant_runs`.':
    case 'Assistant and semantic-index work keep distinct run types.':
      return t('aiIntegrationAuditAssistant')
    case 'MCP and skill integration are both disabled in Settings right now.':
      return t('aiIntegrationWarningDisabled')
    case 'Local MCP client configuration snippet for PathKeep.':
    case 'PathKeep MCP client snippet':
      return t('aiIntegrationGeneratedFileMcpPurpose')
    case 'Codex skill starter that teaches an external assistant how to query PathKeep through MCP.':
    case 'Codex skill starter':
      return t('aiIntegrationGeneratedFileSkillPurpose')
    default: {
      const derivedPathMatch = value.match(
        /^Derived AI state lives beside the archive at (.+) and can be cleared\/rebuilt without touching canonical visits\.$/,
      )
      if (derivedPathMatch) {
        return t('aiIntegrationAuditDerivedPath', {
          path: derivedPathMatch[1],
        })
      }
      return value
    }
  }
}

/**
 * Localizes trusted local-host review strings whenever preview fixtures still
 * emit English sentences.
 */
function localizeLocalHostLine(value: string, t: SettingsTranslator): string {
  switch (value) {
    case 'This local host only uses deterministic Core Intelligence read models.':
    case '这个本地宿主只使用 deterministic Core Intelligence read models。':
    case '這個本地宿主只使用 deterministic Core Intelligence read models。':
      return t('externalOutputsLocalHostBoundaryDeterministic')
    case 'Trusted-only cards must stay inside PathKeep-controlled local surfaces.':
    case 'trusted-only 卡片必须留在 PathKeep 控制的本地 surface 内。':
    case 'trusted-only 卡片必须留在 PathKeep 控制的受信任本地 surface 里。':
    case 'trusted-only 卡片必須留在 PathKeep 控制的本地 surface 內。':
    case 'trusted-only 卡片必須留在 PathKeep 控制的受信任本地 surface 裡。':
      return t('externalOutputsLocalHostBoundaryTrusted')
    case 'Public snapshots intentionally omit visit-level identifiers and direct page URLs.':
    case 'public snapshot 仍会保持脱敏，不包含 visit-level URL 或标识字段。':
    case 'public snapshot 仍會保持去識別化，不包含 visit-level URL 或識別欄位。':
      return t('externalOutputsLocalHostBoundaryPublic')
    case 'Review index.html and bundle.json before handing this folder to another trusted local tool.':
      return t('externalOutputsLocalHostManualReview')
    case 'Open index.html from this folder inside a trusted local browser surface.':
      return t('externalOutputsLocalHostManualOpen')
    case 'Rebuild this local snippet whenever scope, window, or locale changes.':
    case '只要 scope、时间窗口或语言发生变化，就重新创建这个本地片段。':
    case '只要 scope、時間視窗或語言改變，就重新建立這個本地片段。':
      return t('externalOutputsLocalHostManualRebuild')
    case 'This local snippet includes trusted-only cards and should not be treated like a public export.':
    case '这个本地片段包含 trusted-only 卡片，不能把它当成公开导出。':
    case '這個本地片段包含 trusted-only 卡片，不能把它當成公開匯出。':
      return t('externalOutputsLocalHostWarningTrusted')
    case 'Core Intelligence snippet that can be opened directly in a local browser.':
      return t('externalOutputsLocalHostPurposeEntry')
    case 'Machine-readable JSON bundle for the same local host artifact.':
      return t('externalOutputsLocalHostPurposeBundle')
    default:
      return value
  }
}

/**
 * Rewrites AI integration preview payloads into Settings-owned localized copy.
 */
export function localizeAiIntegrationPreview(
  preview: AiIntegrationPreview,
  t: SettingsTranslator,
): AiIntegrationPreview {
  return {
    ...preview,
    consentSummary: localizeAiIntegrationLine(preview.consentSummary, t),
    manualSteps: preview.manualSteps.map((step) =>
      localizeAiIntegrationLine(step, t),
    ),
    capabilityNotes: preview.capabilityNotes.map((note) =>
      localizeAiIntegrationLine(note, t),
    ),
    scopeBoundary: preview.scopeBoundary.map((note) =>
      localizeAiIntegrationLine(note, t),
    ),
    auditTrace: preview.auditTrace.map((note) =>
      localizeAiIntegrationLine(note, t),
    ),
    warnings: preview.warnings.map((warning) =>
      localizeAiIntegrationLine(warning, t),
    ),
    generatedFiles: preview.generatedFiles.map((file) => ({
      ...file,
      purpose: localizeAiIntegrationLine(file.purpose, t),
    })),
  }
}

/**
 * Rewrites trusted local-host preview payloads into Settings-owned localized
 * copy whenever fallback fixtures still emit English.
 */
export function localizeIntelligenceLocalHostPreview(
  preview: IntelligenceLocalHostPreview,
  t: SettingsTranslator,
): IntelligenceLocalHostPreview {
  return {
    ...preview,
    boundaryNotes: preview.boundaryNotes.map((note) =>
      localizeLocalHostLine(note, t),
    ),
    manualSteps: preview.manualSteps.map((step) =>
      localizeLocalHostLine(step, t),
    ),
    warnings: preview.warnings.map((warning) =>
      localizeLocalHostLine(warning, t),
    ),
    generatedFiles: preview.generatedFiles.map((file) => ({
      ...file,
      purpose: localizeLocalHostLine(file.purpose, t),
    })),
  }
}
