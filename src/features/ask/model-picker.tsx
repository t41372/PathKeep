/**
 * Which service and model answers next, picked beside the composer.
 *
 * Responsibilities: list each enabled chat service's models (read from the
 * service when the menu opens, so a model loaded a moment ago shows as
 * loaded), keep the current pick listed even when a service cannot be
 * reached, and hand the pick up. Not responsible for remembering it per
 * conversation (`ask-page.tsx` reads that from the saved answers).
 */
import { useQueries } from '@tanstack/react-query'
import { Check, ChevronDown } from 'lucide-react'
import { useState } from 'react'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { contextSize } from '@/features/ai-setup/providers'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { useI18n } from '@/lib/i18n'
import type { AiProviderConfig, AiProviderModel } from '@/lib/types'

export interface ModelChoice {
  providerId: string
  model: string
}

export function ModelPicker({
  services,
  value,
  onChange,
  disabled,
}: {
  services: readonly AiProviderConfig[]
  value: ModelChoice
  onChange: (choice: ModelChoice) => void
  disabled?: boolean
}) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const lists = useQueries({
    queries: services.map((service) => ({
      queryKey: ['ask-models', service.id, service.baseUrl],
      queryFn: () =>
        intelligenceClient.listProviderModels({
          purpose: 'llm',
          requestFormat: service.requestFormat,
          baseUrl: service.baseUrl ?? null,
          providerId: service.id,
        }),
      enabled: open,
      staleTime: 0,
    })),
  })
  const current = services.find((service) => service.id === value.providerId)

  return (
    <DropdownMenu open={open} onOpenChange={setOpen}>
      <DropdownMenuTrigger
        disabled={disabled}
        aria-label={t('ask.composer.model')}
        className="flex min-w-0 items-center gap-1 rounded-md px-1.5 py-0.5 font-mono text-[11px] text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:pointer-events-none"
      >
        <span className="truncate">
          {current ? `${current.name} · ${value.model}` : value.model}
        </span>
        <ChevronDown className="size-3 shrink-0" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="max-h-80 min-w-64">
        {services.map((service, index) => {
          const list = lists[index]
          const models = chatModels(list.data, service, value)
          return (
            <DropdownMenuGroup key={service.id}>
              {index > 0 && <DropdownMenuSeparator />}
              <DropdownMenuLabel className="text-xs text-muted-foreground">
                {service.name}
              </DropdownMenuLabel>
              {models.map((model) => {
                const picked =
                  value.providerId === service.id && value.model === model.id
                return (
                  <DropdownMenuItem
                    key={model.id}
                    onSelect={() =>
                      onChange({ providerId: service.id, model: model.id })
                    }
                    className="flex items-center gap-2"
                  >
                    <Check
                      className={picked ? 'size-3.5' : 'size-3.5 opacity-0'}
                    />
                    <span className="min-w-0 flex-1 truncate font-mono text-xs">
                      {model.id}
                    </span>
                    <span className="shrink-0 text-[11px] text-muted-foreground">
                      {model.loadedContextLength
                        ? t('ask.composer.modelLoaded', {
                            size: contextSize(model.loadedContextLength),
                          })
                        : model.maxContextLength
                          ? t('ask.composer.modelUpTo', {
                              size: contextSize(model.maxContextLength),
                            })
                          : ''}
                    </span>
                  </DropdownMenuItem>
                )
              })}
              {list.isPending && open && (
                <p className="px-2 py-1.5 text-xs text-muted-foreground">
                  {t('ask.composer.modelsLoading')}
                </p>
              )}
              {list.isError && (
                <p className="px-2 py-1.5 text-xs text-muted-foreground">
                  {t('ask.composer.modelsFailed', { service: service.name })}
                </p>
              )}
            </DropdownMenuGroup>
          )
        })}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

/**
 * The service's chat models, loaded first, with its default and the current
 * pick always there: a service that cannot be reached still offers them.
 */
function chatModels(
  listed: AiProviderModel[] | undefined,
  service: AiProviderConfig,
  value: ModelChoice,
): AiProviderModel[] {
  const models = (listed ?? []).filter((model) => model.kind !== 'embedding')
  const known = new Set(models.map((model) => model.id))
  const extra = [
    service.defaultModel,
    value.providerId === service.id ? value.model : '',
  ]
    .filter((id) => id && !known.has(id))
    .filter((id, index, ids) => ids.indexOf(id) === index)
    .map((id) => ({
      id,
      kind: 'chat' as const,
      loaded: null,
      loadedContextLength: null,
      maxContextLength: null,
    }))
  return [...models, ...extra]
}
