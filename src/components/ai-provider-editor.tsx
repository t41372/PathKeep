/**
 * This module renders the provider-editing controls used by the Settings route when configuring optional AI integrations.
 *
 * Why this file exists:
 * - Shared components keep the shell visually and behaviorally consistent instead of making each route invent its own state grammar.
 * - If a primitive or chrome component changes, multiple workflows can shift at once, so the rationale belongs close to the code.
 *
 * Main declarations:
 * - `AiProviderEditorList`
 *
 * Source-of-truth notes:
 * - Visual language comes from `docs/design/design-tokens.md` and the route/shell structure in `docs/design/screens-and-nav.md`.
 * - Loading, empty, error, permission, and callout behavior must stay aligned with `docs/design/ux-principles.md`.
 */

import type {
  AiProviderConfig,
  AiProviderConnectionTestReport,
  AiProviderPurpose,
  AiRequestFormat,
} from '../lib/types'
import { FieldBlock, ToggleRow } from './ui'

// Stryker disable ArrayDeclaration: option count and labels are asserted in ai-provider-editor.test; this top-level tuple is cached by the Vitest mutation runner.
const aiRequestFormats: AiRequestFormat[] = [
  'openai',
  'anthropic',
  'google',
  'ollama',
  'lm-studio',
]
// Stryker restore ArrayDeclaration

// Presets offered at "Add provider". LM Studio leads because the product's
// headline value is a local LM Studio endpoint; the rest follow the same order
// as the API-format select so the chooser reads consistently.
// Stryker disable ArrayDeclaration: option order/labels asserted in ai-provider-editor.test.
const aiProviderPresets: AiRequestFormat[] = [
  'lm-studio',
  'ollama',
  'openai',
  'anthropic',
  'google',
]
// Stryker restore ArrayDeclaration

/**
 * Explains how ai provider editor list works.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
export function AiProviderEditorList({
  addLabel,
  apiKeys,
  builtInProviderIds,
  builtInBadgeLabel,
  disabled = false,
  formatLabel,
  presetLabel,
  presetLabels,
  onAdd,
  onApiKeyChange,
  onClearKey,
  onClearKeyDisabled,
  onCommit,
  onProbe,
  onProbeDisabled,
  onProbeDisabledHint,
  onRemove,
  onSaveKey,
  onSaveKeyDisabled,
  onSaveKeyDisabledHint,
  onSelect,
  onUpdate,
  providerProbes,
  providers,
  purpose,
  selectedProviderId,
  testingProviderId,
  title,
  translations,
}: {
  addLabel: string
  apiKeys: Record<string, string>
  /**
   * Provider ids that are built-in and must not be deletable or require an API key.
   * Cards for these providers suppress the Remove button and the API-key section,
   * and show `builtInBadgeLabel` in the header instead.
   */
  builtInProviderIds?: string[]
  /**
   * Text shown as a badge on the header of built-in provider cards (e.g. "Built-in · Recommended").
   * Only rendered when `builtInProviderIds` is non-empty.
   */
  builtInBadgeLabel?: string
  disabled?: boolean
  formatLabel: (latency: number, model: string) => string
  presetLabel: string
  presetLabels: Record<AiRequestFormat, string>
  onAdd: (format: AiRequestFormat) => void
  onApiKeyChange: (providerId: string, value: string) => void
  onClearKey: (providerId: string) => void
  onClearKeyDisabled?: (providerId: string) => boolean
  // Commit any in-progress field edits for a provider card when focus leaves it.
  // The editor keeps text edits local (via onUpdate) while typing so saveConfig
  // never runs on the keystroke hot path; this fires on blur so the all-auto-save
  // page persists the finished value. React's onBlur bubbles, so one handler per
  // card catches a blur from any field inside it.
  onCommit?: (providerId: string) => void
  onProbe?: (providerId: string) => void
  onProbeDisabled?: (providerId: string) => boolean
  // Returns a short inline reason when the probe is disabled for a recoverable
  // reason (e.g. the provider is not saved yet) so the button is never a silent
  // dead end. Returns null when there is nothing actionable to say.
  onProbeDisabledHint?: (providerId: string) => string | null
  onRemove: (providerId: string) => void
  onSaveKey: (providerId: string) => void
  onSaveKeyDisabled?: (providerId: string) => boolean
  // Returns a short inline reason when Save key is disabled for a recoverable
  // reason (e.g. the provider is not persisted yet, so the backend can't store a
  // key under an id it has never seen) so the button is never a silent dead end.
  // Returns null when there is nothing actionable to say (e.g. the field is empty).
  onSaveKeyDisabledHint?: (providerId: string) => string | null
  onSelect: (providerId: string) => void
  onUpdate: (providerId: string, patch: Partial<AiProviderConfig>) => void
  providerProbes?: Record<string, AiProviderConnectionTestReport>
  providers: AiProviderConfig[]
  purpose: AiProviderPurpose
  selectedProviderId: string | null
  testingProviderId?: string | null
  title: string
  translations: {
    providerName: string
    providerId: string
    requestFormat: string
    baseUrl: string
    baseUrlPlaceholder: string
    defaultModel: string
    modelCatalog: string
    modelCatalogHint: string
    enabled: string
    temperature: string
    maxTokens: string
    dimensions: string
    notes: string
    apiKey: string
    apiKeyPlaceholder: string
    keySaved: string
    keyNotSaved: string
    saveKey: string
    clearKey: string
    remove: string
    testConnection: string
    testingConnection: string
    probeReachable: string
    probeUnreachable: string
    requestFormatLabels: Record<AiRequestFormat, string>
  }
}) {
  return (
    <div className="surfaceInset providerPanel">
      <div className="toolbarLine">
        <h3>{title}</h3>
        {/*
          Add-provider is a preset chooser, not a single button: the user picks
          the API shape they want to seed (LM Studio first, since local LM Studio
          is the headline path) and the route hook expands it into a truthful
          provider draft via makeDefaultAiProviderDraft. The disabled first option
          is the visible "Add provider" prompt; the select's aria-label names the
          control for assistive tech.
        */}
        <select
          aria-label={presetLabel}
          className="secondaryButton providerPresetPicker"
          disabled={disabled}
          value=""
          onChange={(event) => {
            const format = event.target.value as AiRequestFormat
            if (format) {
              onAdd(format)
              // Reset so re-picking the same preset fires onChange again.
              event.target.value = ''
            }
          }}
        >
          <option value="" disabled>
            {addLabel}
          </option>
          {aiProviderPresets.map((format) => (
            <option key={format} value={format}>
              {presetLabels[format]}
            </option>
          ))}
        </select>
      </div>
      {providers.length ? (
        <div className="providerList">
          {providers.map((provider) => {
            const isBuiltIn = builtInProviderIds?.includes(provider.id) ?? false
            // Built-in providers (the static in-app tier) have a canonical identity the backend
            // re-asserts on every reload, so editing their config fields is a confusing no-op that
            // silently reverts. Render those fields visible-but-locked so the user can SEE the
            // configuration for transparency but cannot push the built-in into a transiently-broken
            // state. Selection (the radio) stays interactive so the user can still pick it.
            const fieldsDisabled = disabled || isBuiltIn
            return (
              <article
                className={`providerCard ${selectedProviderId === provider.id ? 'selected' : ''}`}
                key={provider.id}
                onBlur={
                  onCommit
                    ? (event) => {
                        // Only commit when focus actually leaves this card (not when
                        // it moves between two fields of the same provider), so the
                        // all-auto-save persist runs once per editing session, not
                        // on every field-to-field tab.
                        if (
                          !event.currentTarget.contains(
                            event.relatedTarget as Node | null,
                          )
                        ) {
                          onCommit(provider.id)
                        }
                      }
                    : undefined
                }
              >
                <div className="providerHeader">
                  <label className="providerSelect">
                    <input
                      checked={selectedProviderId === provider.id}
                      disabled={disabled}
                      name={`${purpose}-provider`}
                      type="radio"
                      onChange={() => onSelect(provider.id)}
                    />
                    <strong>{provider.name || provider.id}</strong>
                  </label>
                  {isBuiltIn ? (
                    builtInBadgeLabel ? (
                      <span
                        className="providerBuiltInBadge"
                        data-testid={`provider-builtin-badge-${provider.id}`}
                      >
                        {builtInBadgeLabel}
                      </span>
                    ) : null
                  ) : (
                    <button
                      className="ghostButton"
                      type="button"
                      disabled={disabled}
                      onClick={() => onRemove(provider.id)}
                    >
                      {translations.remove}
                    </button>
                  )}
                </div>

                <div className="fieldGrid two">
                  <FieldBlock
                    label={translations.providerName}
                    control={
                      <input
                        disabled={fieldsDisabled}
                        value={provider.name}
                        onChange={(event) =>
                          onUpdate(provider.id, { name: event.target.value })
                        }
                      />
                    }
                  />
                  <FieldBlock
                    label={translations.providerId}
                    control={
                      <div className="readOnlyField providerMono">
                        {provider.id}
                      </div>
                    }
                  />
                  <FieldBlock
                    label={translations.requestFormat}
                    control={
                      <select
                        disabled={fieldsDisabled}
                        value={provider.requestFormat}
                        onChange={(event) =>
                          onUpdate(provider.id, {
                            requestFormat: event.target
                              .value as AiRequestFormat,
                          })
                        }
                      >
                        {aiRequestFormats.map((format) => (
                          <option key={format} value={format}>
                            {translations.requestFormatLabels[format]}
                          </option>
                        ))}
                      </select>
                    }
                  />
                  <FieldBlock
                    label={translations.baseUrl}
                    control={
                      <input
                        disabled={fieldsDisabled}
                        placeholder={translations.baseUrlPlaceholder}
                        value={provider.baseUrl ?? ''}
                        onChange={(event) =>
                          onUpdate(provider.id, {
                            baseUrl: event.target.value || null,
                          })
                        }
                      />
                    }
                  />
                  <FieldBlock
                    label={translations.defaultModel}
                    control={
                      <input
                        disabled={fieldsDisabled}
                        value={provider.defaultModel}
                        onChange={(event) =>
                          onUpdate(provider.id, {
                            defaultModel: event.target.value,
                          })
                        }
                      />
                    }
                  />
                  <FieldBlock
                    label={translations.modelCatalog}
                    control={
                      <input
                        disabled={fieldsDisabled}
                        placeholder={translations.modelCatalogHint}
                        value={provider.modelCatalog.join(', ')}
                        onChange={(event) =>
                          onUpdate(provider.id, {
                            modelCatalog: event.target.value
                              .split(',')
                              .map((value) => value.trim())
                              .filter(Boolean),
                          })
                        }
                      />
                    }
                  />
                  {purpose === 'llm' ? (
                    <>
                      <FieldBlock
                        label={translations.temperature}
                        control={
                          <input
                            disabled={fieldsDisabled}
                            max={2}
                            min={0}
                            step={0.1}
                            type="number"
                            value={provider.temperature ?? 0}
                            onChange={(event) =>
                              onUpdate(provider.id, {
                                temperature: Number(event.target.value),
                              })
                            }
                          />
                        }
                      />
                      <FieldBlock
                        label={translations.maxTokens}
                        control={
                          <input
                            disabled={fieldsDisabled}
                            min={1}
                            step={1}
                            type="number"
                            value={provider.maxTokens ?? 1200}
                            onChange={(event) =>
                              onUpdate(provider.id, {
                                maxTokens: Number(event.target.value),
                              })
                            }
                          />
                        }
                      />
                    </>
                  ) : (
                    <FieldBlock
                      label={translations.dimensions}
                      control={
                        <input
                          disabled={fieldsDisabled}
                          min={1}
                          step={1}
                          type="number"
                          value={provider.dimensions ?? 1536}
                          onChange={(event) =>
                            onUpdate(provider.id, {
                              dimensions: Number(event.target.value),
                            })
                          }
                        />
                      }
                    />
                  )}
                </div>

                <FieldBlock
                  label={translations.notes}
                  control={
                    <textarea
                      className="multilineInput"
                      disabled={fieldsDisabled}
                      rows={3}
                      value={provider.notes ?? ''}
                      onChange={(event) =>
                        onUpdate(provider.id, {
                          notes: event.target.value || null,
                        })
                      }
                    />
                  }
                />

                <div className="toggleList compactToggleList">
                  <ToggleRow
                    checked={provider.enabled}
                    disabled={fieldsDisabled}
                    label={translations.enabled}
                    onChange={(checked) =>
                      onUpdate(provider.id, { enabled: checked })
                    }
                  />
                </div>

                {/*
                Built-in providers (e.g. the static in-app embedding tier) are keyless by design:
                they need no API key and cannot be deleted. Their secret row is suppressed entirely
                so the UI never presents a dead "Save key" path for a provider that doesn't use one.
              */}
                {!isBuiltIn && (
                  <div className="providerSecretRow">
                    <FieldBlock
                      label={
                        provider.apiKeySaved
                          ? `${translations.apiKey} · ${translations.keySaved}`
                          : `${translations.apiKey} · ${translations.keyNotSaved}`
                      }
                      control={
                        <input
                          autoComplete="off"
                          disabled={disabled}
                          placeholder={translations.apiKeyPlaceholder}
                          type="password"
                          value={apiKeys[provider.id] ?? ''}
                          onChange={(event) =>
                            onApiKeyChange(provider.id, event.target.value)
                          }
                        />
                      }
                    />
                    <div className="toolbarActions">
                      <button
                        className="secondaryButton"
                        type="button"
                        disabled={onSaveKeyDisabled?.(provider.id) ?? false}
                        onClick={() => onSaveKey(provider.id)}
                      >
                        {translations.saveKey}
                      </button>
                      <button
                        className="ghostButton"
                        type="button"
                        disabled={onClearKeyDisabled?.(provider.id) ?? false}
                        onClick={() => onClearKey(provider.id)}
                      >
                        {translations.clearKey}
                      </button>
                      {onProbe ? (
                        <button
                          className="ghostButton"
                          type="button"
                          disabled={onProbeDisabled?.(provider.id) ?? false}
                          onClick={() => onProbe(provider.id)}
                          aria-busy={testingProviderId === provider.id}
                        >
                          {testingProviderId === provider.id ? (
                            <span className="inlineSpinner" aria-hidden="true">
                              <span className="inlineSpinner__dot" />
                              <span className="inlineSpinner__dot" />
                              <span className="inlineSpinner__dot" />
                            </span>
                          ) : null}
                          {testingProviderId === provider.id
                            ? translations.testingConnection
                            : translations.testConnection}
                        </button>
                      ) : null}
                    </div>
                    {/*
                    A disabled Save-key button explains itself the same way the
                    probe does: when the only blocker is that the provider isn't
                    persisted yet (the backend stores the secret by provider id,
                    which doesn't exist in saved config until you Save settings),
                    this inline hint tells the user to save first instead of
                    leaving a dead button. This closes the "I typed a key but it
                    never saved" dead end.
                  */}
                    {onSaveKeyDisabledHint?.(provider.id) ? (
                      <p
                        className="mono-support providerProbeHint"
                        data-testid={`save-key-hint-${provider.id}`}
                      >
                        {onSaveKeyDisabledHint(provider.id)}
                      </p>
                    ) : null}
                    {/*
                    A disabled Test-connection button explains itself: when the
                    only blocker is that the provider isn't persisted yet, this
                    inline hint tells the user to save first instead of leaving a
                    dead button. Rendered as the probe's accessible description.
                  */}
                    {onProbe && onProbeDisabledHint?.(provider.id) ? (
                      <p
                        className="mono-support providerProbeHint"
                        data-testid={`probe-hint-${provider.id}`}
                      >
                        {onProbeDisabledHint(provider.id)}
                      </p>
                    ) : null}
                  </div>
                )}

                {providerProbes?.[provider.id] ? (
                  <div className="result-row providerProbeResult">
                    <div className="result-row__header">
                      <strong>
                        {providerProbes[provider.id].ok
                          ? translations.probeReachable
                          : translations.probeUnreachable}
                      </strong>
                      {providerProbes[provider.id].ok ? (
                        <span className="mono-support">
                          {formatLabel(
                            providerProbes[provider.id].latencyMs,
                            providerProbes[provider.id].model,
                          )}
                        </span>
                      ) : null}
                    </div>
                    <p>{providerProbes[provider.id].message}</p>
                    {providerProbes[provider.id].actionHint ? (
                      <p className="mono-support">
                        {providerProbes[provider.id].actionHint}
                      </p>
                    ) : null}
                  </div>
                ) : null}
              </article>
            )
          })}
        </div>
      ) : (
        <div className="emptyState">{title}</div>
      )}
    </div>
  )
}
