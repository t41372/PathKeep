import { useApp } from '../../lib/app-context'
import { FieldBlock, Surface, ToggleRow } from '../../components/ui'
import { AiProviderEditorList } from '../../components/ai-provider-editor'
import { backend } from '../../lib/backend'

export function AiProvidersSettings() {
  const {
    t,
    draftConfig,
    updateAiSettings,
    addAiProvider,
    updateAiProvider,
    removeAiProvider,
    persistConfig,
    providerSecrets,
    setProviderSecrets,
    runTask,
    setNotice,
    setError,
  } = useApp()

  const aiConfig = draftConfig.ai
  const llmProviders = aiConfig.llmProviders
  const embeddingProviders = aiConfig.embeddingProviders

  async function handleStoreProviderSecret(providerId: string) {
    const apiKey = providerSecrets[providerId]?.trim()
    if (!apiKey) {
      setError(t('enterProviderApiKey'))
      return
    }

    await runTask(t('saveProviderKey'), async () => {
      await backend.storeAiProviderApiKey({ providerId, apiKey })
      setProviderSecrets((c) => ({ ...c, [providerId]: '' }))
      setNotice(t('providerKeyStored'))
    })
  }

  async function handleClearProviderSecret(providerId: string) {
    await runTask(t('clearProviderKey'), async () => {
      await backend.clearAiProviderApiKey(providerId)
      setNotice(t('providerKeyCleared'))
    })
  }

  async function handleSave() {
    await runTask(t('saveSettings'), async () => {
      await persistConfig(draftConfig)
    })
  }

  const providerTranslations = {
    providerName: t('providerName'),
    providerId: t('providerId'),
    requestFormat: t('requestFormat'),
    baseUrl: t('baseUrl'),
    defaultModel: t('defaultModel'),
    modelCatalog: t('modelCatalog'),
    modelCatalogHint: t('modelCatalogHint'),
    enabled: t('providerEnabled'),
    temperature: t('temperature'),
    maxTokens: t('maxTokens'),
    dimensions: t('dimensions'),
    notes: t('notes'),
    apiKey: t('apiKey'),
    keyStored: t('providerKeyStoredState'),
    saveKey: t('saveProviderKey'),
    clearKey: t('clearProviderKey'),
    remove: t('removeProvider'),
  }

  return (
    <div className="settingsTabContent">
      <section className="pageIntro">
        <h2>{t('settingsAiProviders')}</h2>
        <p className="muted">{t('analysisDescription')}</p>
      </section>

      <Surface
        eyebrow={t('analysisSection')}
        title={t('analysisSection')}
        icon="smart_toy"
      >
        <ToggleRow
          label={t('aiEnabled')}
          checked={aiConfig.enabled}
          onChange={(checked) => updateAiSettings({ enabled: checked })}
        />
        {aiConfig.enabled && (
          <>
            <ToggleRow
              label={t('aiAssistantEnabled')}
              checked={aiConfig.assistantEnabled}
              onChange={(checked) =>
                updateAiSettings({ assistantEnabled: checked })
              }
            />
            <ToggleRow
              label={t('semanticIndexEnabled')}
              checked={aiConfig.semanticIndexEnabled}
              onChange={(checked) =>
                updateAiSettings({ semanticIndexEnabled: checked })
              }
            />
            <ToggleRow
              label={t('mcpEnabled')}
              checked={aiConfig.mcpEnabled}
              onChange={(checked) => updateAiSettings({ mcpEnabled: checked })}
            />
            <ToggleRow
              label={t('autoIndexAfterBackup')}
              checked={aiConfig.autoIndexAfterBackup}
              onChange={(checked) =>
                updateAiSettings({ autoIndexAfterBackup: checked })
              }
            />

            <FieldBlock label={t('selectedLlmProvider')}>
              <select
                className="selectInput"
                value={aiConfig.llmProviderId ?? ''}
                onChange={(e) =>
                  updateAiSettings({
                    llmProviderId: e.target.value || null,
                  })
                }
              >
                <option value="">{t('noneSelected')}</option>
                {llmProviders.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
            </FieldBlock>

            <FieldBlock label={t('selectedEmbeddingProvider')}>
              <select
                className="selectInput"
                value={aiConfig.embeddingProviderId ?? ''}
                onChange={(e) =>
                  updateAiSettings({
                    embeddingProviderId: e.target.value || null,
                  })
                }
              >
                <option value="">{t('noneSelected')}</option>
                {embeddingProviders.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
            </FieldBlock>
          </>
        )}
      </Surface>

      {/* LLM Providers */}
      {aiConfig.enabled && (
        <>
          <Surface
            eyebrow={t('llmProvidersTitle')}
            title={t('llmProvidersTitle')}
            icon="psychology"
          >
            <AiProviderEditorList
              addLabel={t('addProvider')}
              apiKeys={providerSecrets}
              onAdd={() => addAiProvider('llm')}
              onApiKeyChange={(id, value) =>
                setProviderSecrets((c) => ({ ...c, [id]: value }))
              }
              onClearKey={handleClearProviderSecret}
              onRemove={(id) => removeAiProvider('llm', id)}
              onSaveKey={handleStoreProviderSecret}
              onSelect={(id) => updateAiSettings({ llmProviderId: id })}
              onUpdate={(id, patch) => updateAiProvider('llm', id, patch)}
              providers={llmProviders}
              purpose="llm"
              selectedProviderId={aiConfig.llmProviderId ?? null}
              title={t('llmProvidersTitle')}
              translations={providerTranslations}
            />
          </Surface>

          <Surface
            eyebrow={t('embeddingProvidersTitle')}
            title={t('embeddingProvidersTitle')}
            icon="data_array"
          >
            <AiProviderEditorList
              addLabel={t('addProvider')}
              apiKeys={providerSecrets}
              onAdd={() => addAiProvider('embedding')}
              onApiKeyChange={(id, value) =>
                setProviderSecrets((c) => ({ ...c, [id]: value }))
              }
              onClearKey={handleClearProviderSecret}
              onRemove={(id) => removeAiProvider('embedding', id)}
              onSaveKey={handleStoreProviderSecret}
              onSelect={(id) => updateAiSettings({ embeddingProviderId: id })}
              onUpdate={(id, patch) => updateAiProvider('embedding', id, patch)}
              providers={embeddingProviders}
              purpose="embedding"
              selectedProviderId={aiConfig.embeddingProviderId ?? null}
              title={t('embeddingProvidersTitle')}
              translations={providerTranslations}
            />
          </Surface>
        </>
      )}

      <div className="settingsActions">
        <button className="primaryButton" type="button" onClick={handleSave}>
          {t('saveSettings')}
        </button>
      </div>
    </div>
  )
}
