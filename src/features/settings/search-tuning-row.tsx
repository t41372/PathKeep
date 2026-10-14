/**
 * Settings → AI → Search ranking: the hybrid-search knobs (RRF `k`, the two
 * list weights, the starred boost), each with its default, its range and a
 * plain explanation.
 *
 * Edits stay a local draft until Save, so typing never writes the config on
 * every keystroke. Ranges match the clamp the backend applies on load
 * (`AiSettings::normalize_search_knobs`), so a saved value is never silently
 * changed afterwards. Not responsible for the search itself.
 */
import { useState } from 'react'
import { toast } from 'sonner'
import { SettingRow } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { useI18n } from '@/lib/i18n'
import { useSnapshot } from '@/lib/queries/app'
import type { AiSettings } from '@/lib/types'
import { useSaveSetting } from './use-save-setting'

type Field = 'hybridRrfK' | 'lexicalWeight' | 'semanticWeight' | 'starredBoost'

interface FieldSpec {
  field: Field
  copy: 'k' | 'lexical' | 'semantic' | 'starred'
  fallback: number
  min: number
  max: number
  step: number
  integer?: boolean
}

/** Defaults and bounds from `vault_core::models::intelligence::AiSettings`. */
const fields: FieldSpec[] = [
  {
    field: 'hybridRrfK',
    copy: 'k',
    fallback: 60,
    min: 1,
    max: 1000,
    step: 1,
    integer: true,
  },
  {
    field: 'lexicalWeight',
    copy: 'lexical',
    fallback: 1,
    min: 0,
    max: 100,
    step: 0.1,
  },
  {
    field: 'semanticWeight',
    copy: 'semantic',
    fallback: 1,
    min: 0,
    max: 100,
    step: 0.1,
  },
  {
    field: 'starredBoost',
    copy: 'starred',
    fallback: 0.15,
    min: 0,
    max: 0.5,
    step: 0.05,
  },
]

type Values = Record<Field, number>

/**
 * The backend stores the weights as f32 and widens them to f64 on the way
 * out (0.3 arrives as 0.30000001192…); six significant digits is the f32's
 * real precision and gives back what the user typed.
 */
function fromBackend(value: number) {
  return Number(value.toPrecision(6))
}

function savedValues(ai: AiSettings): Values {
  return Object.fromEntries(
    fields.map((spec) => [
      spec.field,
      fromBackend(ai[spec.field] ?? spec.fallback),
    ]),
  ) as Values
}

const defaults = Object.fromEntries(
  fields.map((spec) => [spec.field, spec.fallback]),
) as Values

function parse(spec: FieldSpec, text: string): number | null {
  if (text.trim() === '') return null
  const value = Number(text)
  if (!Number.isFinite(value) || value < spec.min || value > spec.max)
    return null
  if (spec.integer && !Number.isInteger(value)) return null
  return value
}

function asText(values: Values) {
  return Object.fromEntries(
    fields.map((spec) => [spec.field, String(values[spec.field])]),
  ) as Record<Field, string>
}

function sameValues(a: Values, b: Values) {
  return fields.every((spec) => a[spec.field] === b[spec.field])
}

export function SearchTuningRow() {
  const ai = useSnapshot().config.ai
  const saved = savedValues(ai)
  // Remount on every saved change, so the draft starts from what was stored.
  return (
    <SearchTuningForm
      key={JSON.stringify(saved)}
      saved={saved}
      semanticOn={ai.enabled && ai.semanticIndexEnabled}
    />
  )
}

function SearchTuningForm({
  saved,
  semanticOn,
}: {
  saved: Values
  semanticOn: boolean
}) {
  const { t } = useI18n()
  const { save, saving } = useSaveSetting()
  const [draft, setDraft] = useState(() => asText(saved))

  const parsed = Object.fromEntries(
    fields.map((spec) => [spec.field, parse(spec, draft[spec.field])]),
  ) as Record<Field, number | null>
  const valid = fields.every((spec) => parsed[spec.field] !== null)
  const dirty = valid && !sameValues(parsed as Values, saved)
  const atDefaults = sameValues(saved, defaults)
  const draftIsSaved = fields.every(
    (spec) => draft[spec.field] === String(saved[spec.field]),
  )

  const write = async (values: Values) => {
    const ok = await save((config) => {
      for (const spec of fields) config.ai[spec.field] = values[spec.field]
      return config
    })
    if (ok) toast.success(t('settingsAi.tuning.saved'))
  }

  return (
    <SettingRow
      title={t('settingsAi.tuning.title')}
      description={
        <span className="flex flex-col">
          <span>{t('settingsAi.tuning.description')}</span>
          {!semanticOn && <span>{t('settingsAi.tuning.needsSemantic')}</span>}
        </span>
      }
    >
      <form
        className="flex flex-col gap-3 border-t pt-3"
        onSubmit={(event) => {
          event.preventDefault()
          if (dirty) void write(parsed as Values)
        }}
      >
        {fields.map((spec) => {
          const id = `settings-tuning-${spec.copy}`
          const invalid = parsed[spec.field] === null
          return (
            <div key={spec.field} className="flex items-start gap-4">
              <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                <label htmlFor={id} className="text-[13px] font-medium">
                  {t(`settingsAi.tuning.${spec.copy}.label`)}
                </label>
                <span className="text-xs text-muted-foreground">
                  {t(`settingsAi.tuning.${spec.copy}.help`)}
                </span>
                <span
                  id={`${id}-hint`}
                  className={
                    invalid
                      ? 'text-xs text-destructive'
                      : 'text-xs text-muted-foreground'
                  }
                >
                  {invalid
                    ? t('settingsAi.tuning.invalid', {
                        min: spec.min,
                        max: spec.max,
                      })
                    : t('settingsAi.tuning.range', {
                        value: spec.fallback,
                        min: spec.min,
                        max: spec.max,
                      })}
                </span>
              </div>
              <Input
                id={id}
                type="number"
                inputMode="decimal"
                min={spec.min}
                max={spec.max}
                step={spec.step}
                value={draft[spec.field]}
                aria-invalid={invalid}
                aria-describedby={`${id}-hint`}
                disabled={saving}
                className="h-8 w-24 shrink-0 text-right tabular-nums"
                onChange={(event) =>
                  setDraft((current) => ({
                    ...current,
                    [spec.field]: event.target.value,
                  }))
                }
              />
            </div>
          )
        })}
        <div className="flex justify-end gap-2">
          <Button
            type="button"
            size="sm"
            variant="ghost"
            disabled={saving || (atDefaults && draftIsSaved)}
            onClick={() => {
              setDraft(asText(defaults))
              if (!atDefaults) void write(defaults)
            }}
          >
            {t('settingsAi.tuning.reset')}
          </Button>
          <Button type="submit" size="sm" disabled={saving || !dirty}>
            {t('settingsAi.tuning.save')}
          </Button>
        </div>
      </form>
    </SettingRow>
  )
}
