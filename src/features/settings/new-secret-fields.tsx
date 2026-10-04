/**
 * "New secret" + "type it again" fields with the length and match checks.
 * Shared by the archive password and the app passcode dialogs, which differ
 * only in labels and minimum length.
 */
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useI18n } from '@/lib/i18n'
import { secretProblem, type NewSecret } from './new-secret'

export function NewSecretFields({
  idPrefix,
  label,
  secret,
  minLength,
  onChange,
  disabled,
  autoFocus = true,
}: {
  idPrefix: string
  label: string
  secret: NewSecret
  minLength: number
  onChange: (secret: NewSecret) => void
  disabled?: boolean
  /** Off when a field above it (the current password) takes focus first. */
  autoFocus?: boolean
}) {
  const { t } = useI18n()
  const problem = secretProblem(secret, minLength)
  // Only complain once the user has typed in the field the problem is about.
  const message =
    problem === 'tooShort' && secret.value.length > 0
      ? t('settings.secret.tooShort', { count: minLength })
      : problem === 'mismatch' && secret.again.length > 0
        ? t('settings.secret.mismatch')
        : null

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-col gap-1.5">
        <Label htmlFor={`${idPrefix}-new`}>{label}</Label>
        <Input
          id={`${idPrefix}-new`}
          type="password"
          autoComplete="new-password"
          autoFocus={autoFocus}
          disabled={disabled}
          value={secret.value}
          onChange={(event) =>
            onChange({ ...secret, value: event.target.value })
          }
        />
      </div>
      <div className="flex flex-col gap-1.5">
        <Label htmlFor={`${idPrefix}-again`}>
          {t('settings.secret.again')}
        </Label>
        <Input
          id={`${idPrefix}-again`}
          type="password"
          autoComplete="new-password"
          disabled={disabled}
          value={secret.again}
          aria-invalid={problem === 'mismatch' && secret.again.length > 0}
          onChange={(event) =>
            onChange({ ...secret, again: event.target.value })
          }
        />
      </div>
      <p
        aria-live="polite"
        className="min-h-[18px] text-[13px] text-muted-foreground"
      >
        {message ?? t('settings.secret.minLength', { count: minLength })}
      </p>
    </div>
  )
}
