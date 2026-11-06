/**
 * The "Words" tab of Advanced search: Google-style fields for every
 * full-text operator the backend parses. Controlled; the query it makes is
 * written by `composeWordsQuery`.
 */
import { useId } from 'react'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useI18n } from '@/lib/i18n'
import type { WordsForm } from './words-query'

type TextField = Exclude<keyof WordsForm, 'starred'>

/** The site field's example: a domain reads the same in every language. */
const SITE_EXAMPLE = 'github.com'

const wordFields: readonly TextField[] = ['all', 'phrase', 'any', 'none']
const narrowFields: readonly TextField[] = [
  'site',
  'title',
  'url',
  'fileType',
  'tags',
  'note',
]

function Field({
  name,
  form,
  onChange,
}: {
  name: TextField
  form: WordsForm
  onChange: (form: WordsForm) => void
}) {
  const { t } = useI18n()
  const id = `${useId()}-${name}`
  const hint =
    name === 'tags' ? t('historySearch.builder.words.tagsHint') : null
  return (
    <div className="grid grid-cols-[170px_1fr] items-center gap-x-3 gap-y-1">
      <Label htmlFor={id} className="text-[13px] font-normal">
        {t(`historySearch.builder.words.${name}`)}
      </Label>
      <Input
        id={id}
        value={form[name]}
        onChange={(event) => onChange({ ...form, [name]: event.target.value })}
        placeholder={
          name === 'site'
            ? SITE_EXAMPLE
            : t(`historySearch.builder.words.examples.${name}`)
        }
        aria-describedby={hint ? `${id}-hint` : undefined}
        autoComplete="off"
        spellCheck={false}
        className="h-8 text-[13px]"
      />
      {hint && (
        <p
          id={`${id}-hint`}
          className="col-start-2 text-[11px] text-muted-foreground"
        >
          {hint}
        </p>
      )}
    </div>
  )
}

export function WordsFields({
  form,
  onChange,
}: {
  form: WordsForm
  onChange: (form: WordsForm) => void
}) {
  const { t } = useI18n()
  const starredId = useId()
  return (
    <div className="flex flex-col gap-2.5">
      <p className="text-xs text-muted-foreground">
        {t('historySearch.builder.words.intro')}
      </p>
      {wordFields.map((name) => (
        <Field key={name} name={name} form={form} onChange={onChange} />
      ))}
      <h3 className="pt-2 text-xs font-medium text-muted-foreground">
        {t('historySearch.builder.words.narrow')}
      </h3>
      {narrowFields.map((name) => (
        <Field key={name} name={name} form={form} onChange={onChange} />
      ))}
      <div className="flex items-center gap-2 pt-1">
        <Checkbox
          id={starredId}
          checked={form.starred}
          onCheckedChange={(checked) =>
            onChange({ ...form, starred: checked === true })
          }
        />
        <Label htmlFor={starredId} className="text-[13px] font-normal">
          {t('historySearch.builder.words.starred')}
        </Label>
      </div>
    </div>
  )
}
