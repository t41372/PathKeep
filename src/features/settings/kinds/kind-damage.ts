/**
 * The words for a kind whose storage will not open, from what the backend
 * reports: which file failed (role) and when that was found (stage), and
 * whether the conversation repair can help with it.
 *
 * Whatever the backend's reason, it is said one way: the file is damaged or
 * cannot be read. By the time a kind is listed the archive is open, so even
 * a `wrong-key` here means this one file no longer reads with the archive's
 * key: damage, never a wrong password, and nothing to type in.
 */
import type {
  KindFailure,
  KindFailureRole,
  KindFailureStage,
} from '@/lib/backend-client/data-kinds'
import type { MessageKey } from '@/lib/i18n'

const parts: Record<KindFailureRole, MessageKey> = {
  evidence: 'dataKinds.damage.parts.evidence',
  'read-model': 'dataKinds.damage.parts.readModel',
  shared: 'dataKinds.damage.parts.shared',
}

const stages: Record<KindFailureStage, MessageKey> = {
  preflight: 'dataKinds.damage.stages.preflight',
  open: 'dataKinds.damage.stages.open',
  'key-check': 'dataKinds.damage.stages.keyCheck',
  migrate: 'dataKinds.damage.stages.migrate',
  recover: 'dataKinds.damage.stages.recover',
  read: 'dataKinds.damage.stages.read',
  write: 'dataKinds.damage.stages.write',
}

/** The sentences for one failure; a role or stage this build does not know falls back to plain words. */
export function damageWords(failure: KindFailure | undefined): MessageKey[] {
  const part = (failure && parts[failure.role]) ?? 'dataKinds.damage.generic'
  const stage = failure ? stages[failure.stage] : undefined
  return stage ? [part, stage] : [part]
}

/**
 * Whether "Repair conversations…" can help. The repair replaces only the two
 * conversation files, so a file shared with the rest of the archive is out of
 * its reach (`conversation-repair-requires-isolated-damage`), and files a newer
 * PathKeep wrote need that PathKeep, not a repair (`conversation-schema-too-new`).
 * Without a reported failure the backend decides when asked.
 */
export function canRepair(failure: KindFailure | undefined) {
  return (
    failure === undefined ||
    (failure.role !== 'shared' && failure.reason !== 'schema-too-new')
  )
}
