/**
 * How the reader puts numbers into words: durations, token counts, a
 * tool's name, and copying a line to the clipboard with its notice.
 *
 * Responsibilities: choosing the words. Not responsible for: the copy
 * itself (`agentReader` messages).
 */
import { toast } from 'sonner'
import { toolName } from '@/features/data-kinds/tools'
import type { TokenKinds } from '@/lib/backend-client/agent-chats'
import type { AgentToolId } from '@/lib/backend-client/data-kinds'
import { describeError } from '@/lib/errors'
import type { Formatters, Translator } from '@/lib/i18n'

/** "12 s", "3 min 20 s", "48 min", "2 h 5 min". */
export function durationText(t: Translator, ms: number) {
  const seconds = Math.max(0, Math.round(ms / 1000))
  if (seconds < 60) return t('agentReader.duration.seconds', { count: seconds })
  const minutes = Math.floor(seconds / 60)
  if (minutes < 10 && seconds % 60 > 0)
    return t('agentReader.duration.minutesSeconds', {
      minutes,
      seconds: seconds % 60,
    })
  if (minutes < 60) return t('agentReader.duration.minutes', { count: minutes })
  const hours = Math.floor(minutes / 60)
  return minutes % 60 === 0
    ? t('agentReader.duration.hours', { count: hours })
    : t('agentReader.duration.hoursMinutes', { hours, minutes: minutes % 60 })
}

/** The four kinds added up (`agent-history.md` §1.7). */
export function tokenTotal(tokens: TokenKinds) {
  return tokens.input + tokens.cacheWrite + tokens.cacheRead + tokens.output
}

export function tokensText(t: Translator, f: Formatters, count: number) {
  return t('agentReader.rows.tokens', { count: f.compact(count) })
}

export function toolLabel(t: Translator, tool: AgentToolId) {
  return t(toolName(tool))
}

/** Copies `text`, then says `done`; says so when the clipboard refuses. */
export function copyText(t: Translator, text: string, done: string) {
  void navigator.clipboard.writeText(text).then(
    () => toast.success(done),
    (error: unknown) =>
      toast.error(t('agentReader.resume.copyFailed'), {
        description: describeError(error),
      }),
  )
}
