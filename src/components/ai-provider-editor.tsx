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
  AiProviderPurpose,
  AiRequestFormat,
} from '../lib/types'
import { FieldBlock, ToggleRow } from './ui'

const aiRequestFormats: AiRequestFormat[] = [
  'openai',
  'anthropic',
  'google',
  'ollama',
  'lm-studio',
]

/**
 * Explains how ai provider editor list works.
 *
 * Shared components and primitives are reused across routes, so naming the contract here keeps the design-system and trust-state behavior consistent.
 */
export function AiProviderEditorList({
  addLabel,
  apiKeys,
  disabled = false,
  onAdd,
  onApiKeyChange,
  onClearKey,
  onClearKeyDisabled,
  onRemove,
  onSaveKey,
  onSaveKeyDisabled,
  onSelect,
  onUpdate,
  providers,
  purpose,
  selectedProviderId,
  title,
  translations,
}: {
  addLabel: string
  apiKeys: Record<string, string>
  disabled?: boolean
  onAdd: () => void
  onApiKeyChange: (providerId: string, value: string) => void
  onClearKey: (providerId: string) => void
  onClearKeyDisabled?: (providerId: string) => boolean
  onRemove: (providerId: string) => void
  onSaveKey: (providerId: string) => void
  onSaveKeyDisabled?: (providerId: string) => boolean
  onSelect: (providerId: string) => void
  onUpdate: (providerId: string, patch: Partial<AiProviderConfig>) => void
  providers: AiProviderConfig[]
  purpose: AiProviderPurpose
  selectedProviderId: string | null
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
    requestFormatLabels: Record<AiRequestFormat, string>
  }
}) {
  return (
    <div className="surfaceInset providerPanel">
      <div className="toolbarLine">
        <h3>{title}</h3>
        <button
          className="secondaryButton"
          type="button"
          disabled={disabled}
          onClick={onAdd}
        >
          {addLabel}
        </button>
      </div>
      {providers.length ? (
        <div className="providerList">
          {providers.map((provider) => (
            <article
              className={`providerCard ${selectedProviderId === provider.id ? 'selected' : ''}`}
              key={provider.id}
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
                <button
                  className="ghostButton"
                  type="button"
                  disabled={disabled}
                  onClick={() => onRemove(provider.id)}
                >
                  {translations.remove}
                </button>
              </div>

              <div className="fieldGrid two">
                <FieldBlock
                  label={translations.providerName}
                  control={
                    <input
                      disabled={disabled}
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
                      disabled={disabled}
                      value={provider.requestFormat}
                      onChange={(event) =>
                        onUpdate(provider.id, {
                          requestFormat: event.target.value as AiRequestFormat,
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
                      disabled={disabled}
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
                      disabled={disabled}
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
                      disabled={disabled}
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
                          disabled={disabled}
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
                          disabled={disabled}
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
                        disabled={disabled}
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
                    disabled={disabled}
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
                  disabled={disabled}
                  label={translations.enabled}
                  onChange={(checked) =>
                    onUpdate(provider.id, { enabled: checked })
                  }
                />
              </div>

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
                </div>
              </div>
            </article>
          ))}
        </div>
      ) : (
        <div className="emptyState">{title}</div>
      )}
    </div>
  )
}
