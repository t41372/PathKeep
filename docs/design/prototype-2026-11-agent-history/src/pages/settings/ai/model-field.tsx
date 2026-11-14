/**
 * The model field of an AI service, as src/features/ai-setup/model-field.tsx:
 * the models the service says it has, read once the address (and key, for a
 * cloud service) is filled in, offered in a list; or any name typed in.
 * The list is fake; `listCase` picks what the "service" answers.
 */
import { Check, ChevronsUpDown } from 'lucide-react'
import { useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList } from '@/components/ui/command'
import { Input } from '@/components/ui/input'
import { Popover, PopoverAnchor, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { contextSize, presetFor, type ProviderDraft, type ProviderKind, type Purpose } from './providers'

/** Prototype-only: what the service answers when PathKeep reads its model list. */
export type ModelListCase = 'found' | 'none' | 'otherKind' | 'failed'

interface FakeModel {
  id: string
  kind: 'chat' | 'embedding'
  loaded?: boolean
  loadedContext?: number
  maxContext?: number
}

const fakeModels: Record<ProviderKind, FakeModel[]> = {
  ollama: [
    { id: 'llama3.2:3b', kind: 'chat', maxContext: 131_072 },
    { id: 'qwen2.5:7b', kind: 'chat', maxContext: 32_768 },
    { id: 'nomic-embed-text', kind: 'embedding', maxContext: 8_192 },
  ],
  'lm-studio': [
    { id: 'qwen/qwen3-8b', kind: 'chat', loaded: true, loadedContext: 32_768, maxContext: 131_072 },
    { id: 'google/gemma-3-12b', kind: 'chat', maxContext: 131_072 },
    { id: 'text-embedding-nomic-embed-text-v1.5', kind: 'embedding', loaded: true, loadedContext: 2_048 },
  ],
  openai: [
    { id: 'gpt-4o-mini', kind: 'chat', maxContext: 131_072 },
    { id: 'gpt-4o', kind: 'chat', maxContext: 131_072 },
    { id: 'text-embedding-3-small', kind: 'embedding' },
    { id: 'text-embedding-3-large', kind: 'embedding' },
  ],
  anthropic: [{ id: 'claude-3-5-haiku-latest', kind: 'chat', maxContext: 204_800 }],
  gemini: [{ id: 'gemini-2.0-flash', kind: 'chat', maxContext: 1_048_576 }],
}

const fits = (purpose: Purpose, model: FakeModel) => model.kind === (purpose === 'embedding' ? 'embedding' : 'chat')

export function ModelField({
  id,
  draft,
  purpose = 'llm',
  keySaved,
  placeholder,
  listCase,
  onChange,
}: {
  id: string
  draft: ProviderDraft
  purpose?: Purpose
  keySaved?: boolean
  placeholder?: string
  listCase: ModelListCase
  onChange: (model: string) => void
}) {
  const [open, setOpen] = useState(false)
  const preset = presetFor(draft.kind, purpose)
  const ready = (draft.baseUrl.trim() !== '' || preset.baseUrlHint !== '') && (!preset.needsKey || draft.apiKey.trim() !== '' || Boolean(keySaved))
  const readKey = `${draft.kind}|${draft.baseUrl}|${draft.apiKey.length > 0}|${listCase}`
  const [loaded, setLoaded] = useState<string | null>(null)
  useEffect(() => {
    if (!ready) return
    // Debounced like the app (500 ms), then the "service" answers.
    const timer = window.setTimeout(() => setLoaded(readKey), 1100)
    return () => window.clearTimeout(timer)
  }, [ready, readKey])
  const loading = ready && loaded !== readKey

  const all = loading || listCase === 'failed' || listCase === 'none' ? [] : listCase === 'otherKind' ? fakeModels[draft.kind].filter((m) => !fits(purpose, m)) : fakeModels[draft.kind]
  const offered = all.filter((m) => fits(purpose, m))

  return (
    <div className="flex flex-col gap-1.5">
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverAnchor asChild>
          <div className="flex gap-1.5">
            <Input id={id} value={draft.model} placeholder={placeholder} spellCheck={false} autoComplete="off" onChange={(event) => onChange(event.target.value)} />
            <PopoverTrigger asChild>
              <Button type="button" variant="outline" size="icon" role="combobox" aria-expanded={open} aria-label="瀏覽清單" disabled={offered.length === 0} className="shrink-0">
                <ChevronsUpDown className="size-3.5 opacity-60" aria-hidden />
              </Button>
            </PopoverTrigger>
          </div>
        </PopoverAnchor>
        <PopoverContent align="start" className="w-(--radix-popper-anchor-width) min-w-[320px] p-0">
          <Command>
            <CommandInput placeholder="尋找模型" />
            <CommandList>
              <CommandEmpty>沒有相符的模型。</CommandEmpty>
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
                    <Check className={model.id === draft.model ? 'size-3.5' : 'size-3.5 opacity-0'} aria-hidden />
                    <span className="min-w-0 flex-1 truncate font-mono text-xs">{model.id}</span>
                    <ModelFacts model={model} />
                  </CommandItem>
                ))}
              </CommandGroup>
            </CommandList>
          </Command>
        </PopoverContent>
      </Popover>
      {ready && (
        <p aria-live="polite" className={listCase === 'failed' && !loading ? 'text-xs text-destructive' : 'text-xs text-muted-foreground'}>
          {loading
            ? '正在讀取服務上的模型…'
            : listCase === 'failed'
              ? `讀不到服務上的模型：error sending request for url (${draft.baseUrl || preset.baseUrlHint}/models) 仍然可以輸入名稱。`
              : offered.length > 0
                ? `服務上有 ${offered.length} 個模型。選一個，或輸入名稱。`
                : all.length > 0
                  ? purpose === 'embedding'
                    ? '服務上沒有 embedding 模型。LM Studio 和 Ollama 的 embedding 模型名稱通常帶有「embed」。'
                    : '服務上只有 embedding 模型，沒有聊天模型。如果有，請輸入名稱。'
                  : '服務上還沒有模型。先在那裡下載或載入一個，或輸入名稱。'}
        </p>
      )}
    </div>
  )
}

function ModelFacts({ model }: { model: FakeModel }) {
  const facts: string[] = []
  if (model.loaded) facts.push('已載入')
  if (model.loadedContext) facts.push(`可讀 ${contextSize(model.loadedContext)}`)
  else if (model.maxContext) facts.push(`最多 ${contextSize(model.maxContext)}`)
  if (facts.length === 0) return null
  return <span className="shrink-0 text-xs text-muted-foreground tabular">{facts.join(' · ')}</span>
}
