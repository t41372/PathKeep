/**
 * "New secret" + "type it again" with the length and match checks, for the
 * archive password and the App Lock passcode. Mirrors
 * `src/features/settings/new-secret-fields.tsx` and `new-secret.ts`.
 */
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

export interface NewSecret {
  value: string
  again: string
}

export const emptySecret: NewSecret = { value: '', again: '' }

export function secretProblem(secret: NewSecret, minLength: number) {
  if (secret.value.length < minLength) return 'tooShort' as const
  if (secret.value !== secret.again) return 'mismatch' as const
  return null
}

export function NewSecretFields({ idPrefix, label, secret, minLength, onChange, disabled, autoFocus = true }: { idPrefix: string; label: string; secret: NewSecret; minLength: number; onChange: (secret: NewSecret) => void; disabled?: boolean; autoFocus?: boolean }) {
  const problem = secretProblem(secret, minLength)
  // Only complain once the user has typed in the field the problem is about.
  const message = problem === 'tooShort' && secret.value.length > 0 ? `請至少輸入 ${minLength} 個字元。` : problem === 'mismatch' && secret.again.length > 0 ? '兩次輸入的不一樣。' : null

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-col gap-1.5">
        <Label htmlFor={`${idPrefix}-new`}>{label}</Label>
        <Input id={`${idPrefix}-new`} type="password" autoComplete="new-password" autoFocus={autoFocus} disabled={disabled} value={secret.value} onChange={(event) => onChange({ ...secret, value: event.target.value })} />
      </div>
      <div className="flex flex-col gap-1.5">
        <Label htmlFor={`${idPrefix}-again`}>再輸入一次</Label>
        <Input
          id={`${idPrefix}-again`}
          type="password"
          autoComplete="new-password"
          disabled={disabled}
          value={secret.again}
          aria-invalid={problem === 'mismatch' && secret.again.length > 0}
          onChange={(event) => onChange({ ...secret, again: event.target.value })}
        />
      </div>
      <p aria-live="polite" className="min-h-[18px] text-[13px] text-muted-foreground">
        {message ?? `至少 ${minLength} 個字元。`}
      </p>
    </div>
  )
}
