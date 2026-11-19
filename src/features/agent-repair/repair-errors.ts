/**
 * Plain words for why a check or a repair of saved conversations stopped,
 * keyed on the backend's code (`command_error.rs`), never on its message.
 * The message is added where it carries what the words can't: why a file
 * couldn't be kept, or a copy's own fault. The file a refusal names is shown
 * under these words (`AffectedFile`).
 *
 * Not responsible for: what the dialog does next (`use-agent-repair.ts`).
 */
import {
  CONVERSATION_ACCESS_UNAVAILABLE,
  CONVERSATION_REPAIR_REQUIRED,
  type RepairErrorCode,
} from '@/lib/backend-client/agent-repair'
import { describeError } from '@/lib/errors'
import type { MessageKey, Translator } from '@/lib/i18n'
import { commandErrorCode } from '@/lib/ipc/command-error'

/** Codes that are said in a sentence of their own. */
const plain: Partial<Record<RepairErrorCode, MessageKey>> = {
  'conversation-repair-preview-stale': 'agentRepair.errors.stale',
  'conversation-access-unavailable': 'agentRepair.errors.accessDenied',
  'conversation-copy-no-verified-evidence':
    'agentRepair.errors.noVerifiedEvidence',
  'conversation-evidence-foreign': 'agentRepair.errors.copyForeign',
  'conversation-copy-not-listed': 'agentRepair.errors.copyNotListed',
  'conversation-copy-incomplete': 'agentRepair.errors.copyIncomplete',
  'conversation-schema-too-new': 'agentRepair.errors.schemaTooNew',
  'conversation-repair-healthy': 'agentRepair.errors.healthy',
  'conversation-evidence-readable': 'agentRepair.errors.evidenceReadable',
  'archive-password-required': 'agentRepair.sources.passwordAsked',
  'archive-password-wrong': 'agentRepair.errors.wrongPassword',
  'archive-busy': 'agentRepair.errors.busy',
  'archive-file-write-failed': 'maintenance.writeFailed',
}

/** Codes whose sentence carries the backend's message: it names the file or the fault. */
const withMessage: Partial<Record<RepairErrorCode, MessageKey>> = {
  'conversation-preservation-failed': 'agentRepair.errors.preservation',
  'conversation-copy-invalid-seal': 'agentRepair.errors.copyInvalid',
  'conversation-copy-duplicate-role': 'agentRepair.errors.copyInvalid',
  'conversation-copy-unsafe-path': 'agentRepair.errors.copyInvalid',
  'snapshot-digest-mismatch': 'agentRepair.errors.copyInvalid',
}

function codeOf(error: unknown) {
  return commandErrorCode(error) as RepairErrorCode | null
}

/** Whether a failed move or encryption change waits for a conversation repair. */
export function needsConversationRepair(error: unknown) {
  return commandErrorCode(error) === CONVERSATION_REPAIR_REQUIRED
}

/** Whether it waits for the system to let PathKeep read the conversation files again. */
export function needsConversationAccess(error: unknown) {
  return commandErrorCode(error) === CONVERSATION_ACCESS_UNAVAILABLE
}

/** The error's code when it is one of `codes`. */
export function repairErrorIs(error: unknown, ...codes: RepairErrorCode[]) {
  const code = codeOf(error)
  return code !== null && codes.includes(code)
}

/** What to say for a failed check (`during: 'check'`) or repair. */
export function repairErrorText(
  t: Translator,
  error: unknown,
  during: 'check' | 'repair',
) {
  const code = codeOf(error)
  const message = describeError(
    error,
    during === 'check'
      ? 'preview_agent_history_repair'
      : 'repair_agent_history',
  )
  const own = code && plain[code]
  if (own) return t(own)
  const carried = code && withMessage[code]
  if (carried) return t(carried, { message })
  return t(
    during === 'check'
      ? 'agentRepair.errors.checkFailed'
      : 'agentRepair.errors.repairFailed',
    { message },
  )
}
