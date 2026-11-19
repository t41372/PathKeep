/**
 * A backup that left a kind of data out because it is paused or off: the
 * menu bar's backup, ⌘K's and "Back up everything now" skip such a kind
 * (only a kind's own "Back up now" still backs up a paused one). When the
 * person pressed something and a kind was left out, they hear which and
 * why, briefly; when nothing was backed up at all, they hear that. A kind
 * that is on with nothing chosen to back up is left out too (the backend
 * refuses it), and said the same way.
 *
 * Not responsible for: deciding to skip (the backend does) or a backup
 * skipped for another reason (another backup running, not due yet).
 */
import type { DataKindId } from '@/lib/backend-client/data-kinds'
import type { MessageKey, Translator } from '@/lib/i18n'

export interface KindSkip {
  kind: DataKindId
  reason: 'kind-paused' | 'kind-off' | 'no-source'
}

/**
 * The backend's reason, when it is about the kind's own state: its
 * `backup_skip_reason` (paused, off), or a refusal saying it has nothing
 * chosen to back up.
 */
export function kindSkipReason(
  code: string | null | undefined,
): KindSkip['reason'] | null {
  if (code === 'kind-paused' || code === 'kind-off') return code
  if (code === 'enable-preview-required' || code === 'source-selection-empty')
    return 'no-source'
  return null
}

const sentence: Record<KindSkip['reason'], MessageKey> = {
  'kind-paused': 'shell.backup.skipped.paused',
  'kind-off': 'shell.backup.skipped.off',
  'no-source': 'shell.backup.skipped.noSource',
}

/** "Agent conversations are paused, so they weren't backed up." */
export function kindSkipSentence(t: Translator, skip: KindSkip) {
  const name = t(
    skip.kind === 'browsing' ? 'spaces.browsing.name' : 'spaces.agent.name',
  )
  return t(sentence[skip.reason], { name })
}
