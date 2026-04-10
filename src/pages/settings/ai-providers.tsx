import { useEffect, useState } from 'react'
import { useApp } from '../../lib/app-context'
import {
  DataRow,
  EmptyState,
  InfoStat,
  StatusTag,
  Surface,
  ToggleRow,
  FieldBlock,
} from '../../components/ui'
import { AiProviderEditorList } from '../../components/ai-provider-editor'
import { backend } from '../../lib/backend'
import { formatDateTime } from '../../lib/format'
import {
  enrichmentPluginBoundaryLabel,
  enrichmentPluginDescription,
  enrichmentPluginLabel,
  upsertEnrichmentPluginPreference,
} from '../../lib/intelligence-runtime'
import type { IntelligenceRuntimeSnapshot } from '../../lib/types'

export function AiProvidersSettings() {
  const {
    t,
    resolvedLanguage,
    initialized,
    unlocked,
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
  const [runtimeSnapshot, setRuntimeSnapshot] =
    useState<IntelligenceRuntimeSnapshot | null>(null)
  const [runtimeError, setRuntimeError] = useState<string | null>(null)

  useEffect(() => {
    if (!initialized || !unlocked) return

    let cancelled = false
    void (async () => {
      try {
        const snapshot = await backend.loadIntelligenceRuntime()
        if (!cancelled) {
          setRuntimeSnapshot(snapshot)
          setRuntimeError(null)
        }
      } catch (error) {
        if (!cancelled) {
          setRuntimeSnapshot(null)
          setRuntimeError(
            error instanceof Error ? error.message : String(error),
          )
        }
      }
    })()

    return () => {
      cancelled = true
    }
  }, [initialized, unlocked])

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

  const pluginCards =
    runtimeSnapshot?.plugins.map((plugin) => ({
      ...plugin,
      enabled:
        aiConfig.enrichmentPlugins.find(
          (preference) => preference.pluginId === plugin.pluginId,
        )?.enabled ?? plugin.enabled,
    })) ??
    aiConfig.enrichmentPlugins.map((plugin) => ({
      pluginId: plugin.pluginId,
      sourceKind:
        plugin.pluginId === 'readable-content-refetch' ? 'network' : 'local',
      enabled: plugin.enabled,
      storedRecords: 0,
      queuedJobs: 0,
      runningJobs: 0,
      failedJobs: 0,
      lastCompletedAt: null,
      lastError: null,
    }))

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
            <ToggleRow
              label={t('enrichmentEnabled')}
              checked={aiConfig.enrichmentEnabled}
              onChange={(checked) =>
                updateAiSettings({ enrichmentEnabled: checked })
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

      <Surface
        eyebrow={t('enrichmentRuntimeTitle')}
        title={t('enrichmentRuntimeTitle')}
        icon="hub"
      >
        <p className="muted">{t('enrichmentBoundaryDescription')}</p>

        {!initialized || !unlocked ? (
          <EmptyState message={t('unlockToInspectRuntime')} icon="lock" />
        ) : runtimeError ? (
          <EmptyState message={runtimeError} icon="error" />
        ) : (
          <>
            <div className="runtimeSummaryGrid">
              <InfoStat
                label={t('queuedJobs')}
                value={runtimeSnapshot?.queue.queued ?? 0}
              />
              <InfoStat
                label={t('runningStatus')}
                value={runtimeSnapshot?.queue.running ?? 0}
              />
              <InfoStat
                label={t('failedJobs')}
                value={runtimeSnapshot?.queue.failed ?? 0}
              />
              <InfoStat
                label={t('completedJobs')}
                value={runtimeSnapshot?.queue.succeeded ?? 0}
              />
            </div>

            <div className="pluginRuntimeGrid">
              {pluginCards.map((plugin) => (
                <article className="pluginRuntimeCard" key={plugin.pluginId}>
                  <div className="pluginRuntimeHeader">
                    <div>
                      <div className="pluginRuntimeTitleRow">
                        <strong>
                          {enrichmentPluginLabel(plugin.pluginId, t)}
                        </strong>
                        <StatusTag
                          tone={
                            plugin.sourceKind === 'network' ? 'info' : 'neutral'
                          }
                        >
                          {enrichmentPluginBoundaryLabel(plugin.sourceKind, t)}
                        </StatusTag>
                      </div>
                      <p className="muted">
                        {enrichmentPluginDescription(plugin.pluginId, t)}
                      </p>
                    </div>
                    <ToggleRow
                      checked={plugin.enabled}
                      label={t('providerEnabled')}
                      onChange={(checked) =>
                        updateAiSettings({
                          enrichmentPlugins: upsertEnrichmentPluginPreference(
                            aiConfig.enrichmentPlugins,
                            plugin.pluginId,
                            checked,
                          ),
                        })
                      }
                    />
                  </div>

                  <div className="runtimeStatGrid">
                    <DataRow label={t('storedRecords')}>
                      {plugin.storedRecords}
                    </DataRow>
                    <DataRow label={t('queuedJobs')}>
                      {plugin.queuedJobs}
                    </DataRow>
                    <DataRow label={t('failedJobs')}>
                      {plugin.failedJobs}
                    </DataRow>
                    <DataRow label={t('lastCompleted')}>
                      {formatDateTime(
                        plugin.lastCompletedAt,
                        resolvedLanguage,
                      ) ?? t('notAvailable')}
                    </DataRow>
                  </div>

                  {plugin.lastError ? (
                    <DataRow label={t('lastErrorLabel')}>
                      <StatusTag tone="danger">{plugin.lastError}</StatusTag>
                    </DataRow>
                  ) : null}
                </article>
              ))}
            </div>

            {runtimeSnapshot?.notes.length ? (
              <div className="runtimeNotes">
                {runtimeSnapshot.notes.map((note) => (
                  <p className="muted" key={note}>
                    {note}
                  </p>
                ))}
              </div>
            ) : null}
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
