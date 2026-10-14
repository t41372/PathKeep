/**
 * Actions on the user's AI services, for the assistant (`llm`) or for
 * semantic search's embeddings (`embedding`): add, edit, select, remove,
 * test. Each one saves through `useSaveConfig` so the snapshot stays the
 * single source of truth. Not responsible for any rendering, or for building
 * the search index when the embeddings service changes (`use-semantic-index`).
 */
import { useQueryClient } from '@tanstack/react-query'
import { useCallback, useState } from 'react'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { useI18n } from '@/lib/i18n'
import { queryKeys } from '@/lib/query'
import { useSaveConfig } from '@/lib/queries/app'
import {
  BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID,
  type AiProviderConfig,
  type AiProviderConnectionTestReport,
  type AiProviderPurpose,
  type AiSettings,
} from '@/lib/types'
import {
  draftToProvider,
  newProviderId,
  presetFor,
  type ProviderDraft,
} from './providers'

function masterStaysOn(ai: AiSettings) {
  return ai.semanticIndexEnabled || ai.mcpEnabled || ai.skillEnabled
}

/** The list of services for `purpose` in the config. */
function servicesOf(ai: AiSettings, purpose: AiProviderPurpose) {
  return purpose === 'embedding' ? ai.embeddingProviders : ai.llmProviders
}

function setServices(
  ai: AiSettings,
  purpose: AiProviderPurpose,
  services: AiProviderConfig[],
) {
  if (purpose === 'embedding') ai.embeddingProviders = services
  else ai.llmProviders = services
}

export function useAiProviders(purpose: AiProviderPurpose = 'llm') {
  const { t } = useI18n()
  const client = useQueryClient()
  const save = useSaveConfig()
  const [testing, setTesting] = useState<string | null>(null)
  const [reports, setReports] = useState<
    Record<string, AiProviderConnectionTestReport | Error>
  >({})

  const test = useCallback(
    async (providerId: string) => {
      setTesting(providerId)
      try {
        const report = await intelligenceClient.testProviderConnection({
          providerId,
          purpose,
        })
        setReports((current) => ({ ...current, [providerId]: report }))
      } catch (reason) {
        const error =
          reason instanceof Error ? reason : new Error(String(reason))
        setReports((current) => ({ ...current, [providerId]: error }))
      } finally {
        setTesting(null)
      }
    },
    [purpose],
  )

  const storeKey = useCallback(
    async (providerId: string, apiKey: string) => {
      const key = apiKey.trim()
      if (!key) return
      const snapshot = await intelligenceClient.storeProviderApiKey({
        providerId,
        apiKey: key,
      })
      client.setQueryData(queryKeys.snapshot, snapshot)
    },
    [client],
  )

  /**
   * Saves the service and stores its key, then checks the connection. An
   * assistant service is also selected; an embeddings service is selected by
   * the caller, since that starts a new index.
   */
  const add = useCallback(
    async (draft: ProviderDraft) => {
      const id = newProviderId(draft.kind, purpose)
      const provider = draftToProvider(
        draft,
        id,
        t(`settingsAi.provider.defaultName.${draft.kind}`),
        purpose,
      )
      await save.mutateAsync((config) => {
        setServices(config.ai, purpose, [
          ...servicesOf(config.ai, purpose),
          provider,
        ])
        if (purpose === 'llm') {
          config.ai.llmProviderId = id
          config.ai.enabled = true
          config.ai.assistantEnabled = true
        }
        return config
      })
      await storeKey(id, draft.apiKey)
      void test(id)
      return provider
    },
    [purpose, save, storeKey, t, test],
  )

  /**
   * Changes a saved service in place: what the form shows, and the key when a
   * new one was typed. Settings the form does not show are kept.
   */
  const update = useCallback(
    async (providerId: string, draft: ProviderDraft) => {
      const preset = presetFor(draft.kind, purpose)
      const model = draft.model.trim()
      const snapshot = await save.mutateAsync((config) => {
        setServices(
          config.ai,
          purpose,
          servicesOf(config.ai, purpose).map((provider) =>
            provider.id === providerId
              ? {
                  ...provider,
                  name: draft.name.trim() || provider.name,
                  requestFormat: preset.requestFormat,
                  baseUrl: draft.baseUrl.trim() || null,
                  defaultModel: model,
                  modelCatalog: model ? [model] : [],
                }
              : provider,
          ),
        )
        return config
      })
      const updated = servicesOf(snapshot.config.ai, purpose).find(
        (provider) => provider.id === providerId,
      )
      if (!updated) throw new Error(`No service ${providerId} to change.`)
      await storeKey(providerId, draft.apiKey)
      void test(providerId)
      return updated
    },
    [purpose, save, storeKey, test],
  )

  const select = useCallback(
    (providerId: string | null) =>
      save.mutateAsync((config) => {
        config.ai.llmProviderId = providerId
        config.ai.assistantEnabled = providerId !== null
        config.ai.enabled = providerId !== null || masterStaysOn(config.ai)
        return config
      }),
    [save],
  )

  const remove = useCallback(
    async (providerId: string) => {
      await intelligenceClient
        .clearProviderApiKey(providerId)
        .catch(() => undefined)
      await save.mutateAsync((config) => {
        setServices(
          config.ai,
          purpose,
          servicesOf(config.ai, purpose).filter(
            (item) => item.id !== providerId,
          ),
        )
        if (purpose === 'llm' && config.ai.llmProviderId === providerId) {
          config.ai.llmProviderId = null
          config.ai.assistantEnabled = false
          config.ai.enabled = masterStaysOn(config.ai)
        }
        // Semantic search goes back to the model that comes with the app.
        if (
          purpose === 'embedding' &&
          config.ai.embeddingProviderId === providerId
        ) {
          config.ai.embeddingProviderId = BUILT_IN_STATIC_EMBEDDING_PROVIDER_ID
        }
        return config
      })
      setReports((reports) => {
        const next = { ...reports }
        delete next[providerId]
        return next
      })
    },
    [purpose, save],
  )

  return {
    purpose,
    add,
    update,
    select,
    remove,
    test,
    testing,
    reports,
    saving: save.isPending,
  }
}
