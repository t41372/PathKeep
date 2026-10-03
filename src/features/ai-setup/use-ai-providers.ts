/**
 * Actions on the user's AI services: add, select, remove, test.
 * Each one saves through `useSaveConfig` so the snapshot stays the single
 * source of truth. Not responsible for any rendering.
 */
import { useQueryClient } from '@tanstack/react-query'
import { useCallback, useState } from 'react'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { useI18n } from '@/lib/i18n'
import { queryKeys } from '@/lib/query'
import { useSaveConfig } from '@/lib/queries/app'
import type { AiProviderConnectionTestReport, AiSettings } from '@/lib/types'
import { draftToProvider, newProviderId, type ProviderDraft } from './providers'

function masterStaysOn(ai: AiSettings) {
  return ai.semanticIndexEnabled || ai.mcpEnabled || ai.skillEnabled
}

export function useAiProviders() {
  const { t } = useI18n()
  const client = useQueryClient()
  const save = useSaveConfig()
  const [testing, setTesting] = useState<string | null>(null)
  const [reports, setReports] = useState<
    Record<string, AiProviderConnectionTestReport | Error>
  >({})

  const test = useCallback(async (providerId: string) => {
    setTesting(providerId)
    try {
      const report = await intelligenceClient.testProviderConnection({
        providerId,
        purpose: 'llm',
      })
      setReports((current) => ({ ...current, [providerId]: report }))
    } catch (reason) {
      const error = reason instanceof Error ? reason : new Error(String(reason))
      setReports((current) => ({ ...current, [providerId]: error }))
    } finally {
      setTesting(null)
    }
  }, [])

  /** Saves the service, stores its key, selects it, and checks the connection. */
  const add = useCallback(
    async (draft: ProviderDraft) => {
      const id = newProviderId(draft.kind)
      const provider = draftToProvider(
        draft,
        id,
        t(`settingsAi.provider.defaultName.${draft.kind}`),
      )
      await save.mutateAsync((config) => {
        config.ai.llmProviders = [...config.ai.llmProviders, provider]
        config.ai.llmProviderId = id
        config.ai.enabled = true
        config.ai.assistantEnabled = true
        return config
      })
      const key = draft.apiKey.trim()
      if (key) {
        const snapshot = await intelligenceClient.storeProviderApiKey({
          providerId: id,
          apiKey: key,
        })
        client.setQueryData(queryKeys.snapshot, snapshot)
      }
      void test(id)
      return provider
    },
    [client, save, t, test],
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
        config.ai.llmProviders = config.ai.llmProviders.filter(
          (item) => item.id !== providerId,
        )
        if (config.ai.llmProviderId === providerId) {
          config.ai.llmProviderId = null
          config.ai.assistantEnabled = false
          config.ai.enabled = masterStaysOn(config.ai)
        }
        return config
      })
      setReports(({ [providerId]: _gone, ...rest }) => rest)
    },
    [save],
  )

  return { add, select, remove, test, testing, reports, saving: save.isPending }
}
