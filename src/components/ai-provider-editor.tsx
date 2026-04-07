import type { AiProviderConfig, AiRequestFormat } from '../lib/types'
import { FieldBlock, ToggleRow } from './ui'

const aiRequestFormats: AiRequestFormat[] = [
  'openai',
  'anthropic',
  'google',
  'ollama',
  'lm-studio',
]

export type AiProviderPurpose = 'llm' | 'embedding'

export function AiProviderEditorList({
  addLabel,
  apiKeys,
  onAdd,
  onApiKeyChange,
  onClearKey,
  onRemove,
  onSaveKey,
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
  onAdd: () => void
  onApiKeyChange: (providerId: string, value: string) => void
  onClearKey: (providerId: string) => void
  onRemove: (providerId: string) => void
  onSaveKey: (providerId: string) => void
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
    defaultModel: string
    modelCatalog: string
    modelCatalogHint: string
    enabled: string
    temperature: string
    maxTokens: string
    dimensions: string
    notes: string
    apiKey: string
    keyStored: string
    saveKey: string
    clearKey: string
    remove: string
  }
}) {
  return (
    <div className="surfaceInset providerPanel">
      <div className="toolbarLine">
        <h3>{title}</h3>
        <button className="secondaryButton" type="button" onClick={onAdd}>
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
                    name={`${purpose}-provider`}
                    type="radio"
                    onChange={() => onSelect(provider.id)}
                  />
                  <strong>{provider.name || provider.id}</strong>
                </label>
                <button
                  className="ghostButton"
                  type="button"
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
                      value={provider.requestFormat}
                      onChange={(event) =>
                        onUpdate(provider.id, {
                          requestFormat: event.target.value as AiRequestFormat,
                        })
                      }
                    >
                      {aiRequestFormats.map((format) => (
                        <option key={format} value={format}>
                          {format}
                        </option>
                      ))}
                    </select>
                  }
                />
                <FieldBlock
                  label={translations.baseUrl}
                  control={
                    <input
                      placeholder="https://api.example.com/v1"
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
                  label={translations.enabled}
                  onChange={(checked) =>
                    onUpdate(provider.id, { enabled: checked })
                  }
                />
              </div>

              <div className="providerSecretRow">
                <FieldBlock
                  label={`${translations.apiKey} · ${translations.keyStored}: ${provider.apiKeySaved ? 'yes' : 'no'}`}
                  control={
                    <input
                      autoComplete="off"
                      placeholder="sk-..."
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
                    onClick={() => onSaveKey(provider.id)}
                  >
                    {translations.saveKey}
                  </button>
                  <button
                    className="ghostButton"
                    type="button"
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
