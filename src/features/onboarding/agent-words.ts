/**
 * How onboarding puts the Agent tools' walk and first save into words: why a
 * walk did not finish (the enable dialog's reasons, which mean the same
 * here), and why the first save could not start.
 *
 * Not responsible for: the copy itself (`enableAgent` and `onboarding`
 * messages) or where it is drawn.
 */
import {
  dataKindsErrorCode,
  isStoreDamage,
} from '@/lib/backend-client/data-kinds'
import { describeError } from '@/lib/errors'
import type { MessageKey, Translator } from '@/lib/i18n'
import { PREVIEW_CONNECTION_LOST } from '@/lib/ipc/agent-preview-stream'

function walkFailureKey(code: string | null): MessageKey | null {
  if (code === PREVIEW_CONNECTION_LOST) {
    return 'enableAgent.preview.reasons.connection'
  }
  if (isStoreDamage(code)) return 'enableAgent.preview.reasons.unavailable'
  switch (dataKindsErrorCode(code)) {
    case 'archive-busy':
      return 'enableAgent.preview.reasons.busy'
    case 'destination-unavailable':
      return 'enableAgent.preview.reasons.destination'
    case 'archive-replaced':
      return 'enableAgent.preview.reasons.replaced'
    case 'policy-changed':
      return 'enableAgent.preview.reasons.policyChanged'
    case 'root-changed':
      return 'enableAgent.preview.reasons.rootChanged'
    case 'archive-uninitialized':
      return 'enableAgent.preview.reasons.uninitialized'
    default:
      return null
  }
}

/** Why a walk of the tool folders stopped, from its `error` frame's code. */
export function walkFailureText(t: Translator, code: string | null) {
  const key = walkFailureKey(code)
  return key ? t(key) : t('onboarding.agent.stopped', { code: code ?? '—' })
}

/**
 * A walk that stopped, carried as an error so the first save's steps can
 * fail with it like any other call.
 */
export class WalkStopped extends Error {
  readonly code: string | null
  constructor(code: string | null) {
    super(code ?? 'preview-stopped')
    this.name = 'WalkStopped'
    this.code = code
  }
}

/** Why the first save of Agent conversations could not start, as a sentence. */
export function agentStartFailure(t: Translator, error: unknown) {
  if (error instanceof WalkStopped) return walkFailureText(t, error.code)
  return describeError(error, 'enable_agent_history')
}
