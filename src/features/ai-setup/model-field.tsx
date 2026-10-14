/**
 * The model field of an AI service: the models the service says it has, read
 * once the address (and key, for a cloud service) is filled in, offered in a
 * list with what each is for and how much it can read; or any name typed in.
 *
 * Responsible for: reading the list (`list_ai_provider_models`), offering the
 * models that fit the purpose, and saying when the list could not be read.
 * Not responsible for: saving the choice or testing it (the dialog's).
 */
import { useQuery } from '@tanstack/react-query'
import { Check, ChevronsUpDown } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from '@/components/ui/command'
import { Input } from '@/components/ui/input'
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import { intelligenceClient } from '@/lib/backend-client/intelligence'
import { describeError } from '@/lib/errors'
import { useDebouncedValue } from '@/lib/hooks/use-debounced-value'
import { useI18n } from '@/lib/i18n'
import type {
  AiModelKind,
  AiProviderModel,
  AiProviderPurpose,
} from '@/lib/types'
import { contextSize, presetFor, type ProviderDraft } from './providers'

/** Kinds offered for each purpose; a model the service does not type is offered for both. */
const FITS: Record<AiProviderPurpose, readonly AiModelKind[]> = {
  llm: ['chat', 'unknown'],
  embedding: ['embedding', 'unknown'],
}

export function ModelField({
  id,
  draft,
  purpose = 'llm',
  providerId,
  placeholder,
  onChange,
}: {
  id: string
  draft: ProviderDraft
  purpose?: AiProviderPurpose
  /** The saved service being edited, whose stored key is used when none is typed. */
  providerId?: string | null
  placeholder?: string
  onChange: (model: string) => void
}) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const preset = presetFor(draft.kind, purpose)
  // Read once typing stops; the key itself stays out of the query key.
  const where = useDebouncedValue(
    { format: preset.requestFormat, baseUrl: draft.baseUrl.trim() },
    500,
  )
  const key = useDebouncedValue(draft.apiKey.trim(), 500)
  const ready =
    (where.baseUrl !== '' || preset.baseUrlHint !== '') &&
    (!preset.needsKey || key !== '' || Boolean(providerId))
  const models = useQuery({
    queryKey: [
      'ai-provider-models',
      where.format,
      where.baseUrl,
      providerId ?? null,
      key.length,
    ],
    queryFn: () =>
      intelligenceClient.listProviderModels({
        purpose,
        requestFormat: where.format,
        baseUrl: where.baseUrl || null,
        providerId: providerId ?? null,
        apiKey: key || null,
      }),
    enabled: ready,
    retry: false,
    staleTime: 30_000,
  })
  const offered = (models.data ?? []).filter((model) =>
    FITS[purpose].includes(model.kind),
  )

  return (
    <div className="flex flex-col gap-1.5">
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverAnchor asChild>
          <div className="flex gap-1.5">
            <Input
              id={id}
              value={draft.model}
              placeholder={placeholder}
              spellCheck={false}
              autoComplete="off"
              onChange={(event) => onChange(event.target.value)}
            />
            <PopoverTrigger asChild>
              <Button
                type="button"
                variant="outline"
                size="icon"
                role="combobox"
                aria-expanded={open}
                aria-label={t('settingsAi.provider.models.browse')}
                disabled={offered.length === 0}
                className="shrink-0"
              >
                <ChevronsUpDown className="size-3.5 opacity-60" aria-hidden />
              </Button>
            </PopoverTrigger>
          </div>
        </PopoverAnchor>
        <PopoverContent
          align="start"
          className="w-(--radix-popper-anchor-width) min-w-[320px] p-0"
        >
          <Command>
            <CommandInput
              placeholder={t('settingsAi.provider.models.search')}
            />
            <CommandList>
              <CommandEmpty>
                {t('settingsAi.provider.models.empty')}
              </CommandEmpty>
              <CommandGroup>
                {offered.map((model) => (
                  <CommandItem
                    key={model.id}
                    value={model.id}
                    onSelect={() => {
                      onChange(model.id)
                      setOpen(false)
                    }}
                    className="text-[13px]"
                  >
                    <Check
                      className={
                        model.id === draft.model
                          ? 'size-3.5'
                          : 'size-3.5 opacity-0'
                      }
                      aria-hidden
                    />
                    <span className="min-w-0 flex-1 truncate font-mono text-xs">
                      {model.id}
                    </span>
                    <ModelFacts model={model} />
                  </CommandItem>
                ))}
              </CommandGroup>
            </CommandList>
          </Command>
        </PopoverContent>
      </Popover>
      <ListStatus
        ready={ready}
        loading={models.isFetching}
        error={models.error}
        count={offered.length}
        anyModels={(models.data?.length ?? 0) > 0}
        purpose={purpose}
      />
    </div>
  )
}

/** Loaded or not, and how much it can read: what a choice between models turns on. */
function ModelFacts({ model }: { model: AiProviderModel }) {
  const { t } = useI18n()
  const facts: string[] = []
  if (model.loaded) facts.push(t('settingsAi.provider.models.loaded'))
  if (model.loadedContextLength)
    facts.push(
      t('settingsAi.provider.models.context', {
        size: contextSize(model.loadedContextLength),
      }),
    )
  else if (model.maxContextLength)
    facts.push(
      t('settingsAi.provider.models.upTo', {
        size: contextSize(model.maxContextLength),
      }),
    )
  if (facts.length === 0) return null
  return (
    <span className="shrink-0 text-xs text-muted-foreground tabular">
      {facts.join(' · ')}
    </span>
  )
}

function ListStatus({
  ready,
  loading,
  error,
  count,
  anyModels,
  purpose,
}: {
  ready: boolean
  loading: boolean
  error: unknown
  count: number
  anyModels: boolean
  purpose: AiProviderPurpose
}) {
  const { t } = useI18n()
  if (!ready) return null
  const text = loading
    ? t('settingsAi.provider.models.loading')
    : error
      ? t('settingsAi.provider.models.failed', {
          message: describeError(error, 'list_ai_provider_models'),
        })
      : count > 0
        ? t('settingsAi.provider.models.found', { count })
        : anyModels
          ? t(`settingsAi.provider.models.noneFor.${purpose}`)
          : t('settingsAi.provider.models.none')
  return (
    <p
      aria-live="polite"
      className={
        error && !loading
          ? 'text-xs text-destructive'
          : 'text-xs text-muted-foreground'
      }
    >
      {text}
    </p>
  )
}
