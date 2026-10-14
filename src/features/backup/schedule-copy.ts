/**
 * Turns the scheduler's copy keys (`schedule.verifyMacosLoaded`) into text in
 * the user's language. Keys the catalog does not know yet return null, so
 * the caller can fall back to the raw evidence instead of showing a key.
 */
import { messages, type MessageKey, type Translator } from '@/lib/i18n'

const known = messages.en.backupSchedule.backend
type BackendKey = keyof typeof known

export function scheduleCopy(
  t: Translator,
  key: string | null | undefined,
): string | null {
  if (!key?.startsWith('schedule.')) return null
  const suffix = key.slice('schedule.'.length)
  if (!(suffix in known)) return null
  return t(`backupSchedule.backend.${suffix as BackendKey}` as MessageKey)
}
