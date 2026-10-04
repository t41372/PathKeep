/** A new password or passcode typed twice, and what is wrong with it if anything. */
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
