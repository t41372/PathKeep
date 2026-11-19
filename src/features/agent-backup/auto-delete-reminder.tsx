/**
 * The reminder that Claude Code deletes old conversations by default, on
 * the Agent backup page (`agent-history.md` §6). PathKeep never reads the
 * tool's settings file, so it states the tool's default, not a value read
 * from it; and says how many of its conversations are now only in PathKeep.
 *
 * Responsibilities: the card, its link to Claude Code's own documentation,
 * and "Got it", which puts it away on this computer for good
 * (`use-agent-hint.ts`). Shown only while Claude Code is a source here or
 * has conversations in the archive.
 */
import { ExternalLink, TriangleAlert } from 'lucide-react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { supportClient } from '@/lib/backend-client/support'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { useAgentHint } from '@/features/data-kinds/use-agent-hint'

/** Claude Code's own page on its settings, where `cleanupPeriodDays` is described. */
const DOCS_URL = 'https://docs.anthropic.com/en/docs/claude-code/settings'

export function AutoDeleteReminder({
  onlyInPathkeep,
}: {
  onlyInPathkeep: number
}) {
  const { t } = useI18n()
  const { dismissed, dismiss } = useAgentHint('auto-delete')
  // Unknown until the settings answer: show nothing rather than flash it.
  if (dismissed !== false) return null

  return (
    <section
      aria-label={t('agentBackup.reminder.title')}
      className="flex animate-rise items-start gap-3 rounded-xl border border-brand/25 bg-brand-soft px-5 py-4"
    >
      <TriangleAlert
        aria-hidden
        className="mt-0.5 size-4 shrink-0 text-brand"
      />
      <div className="flex min-w-0 flex-1 flex-col gap-1 text-[13px]">
        <span className="font-medium">{t('agentBackup.reminder.title')}</span>
        <span className="max-w-[72ch] leading-relaxed text-muted-foreground">
          {t('agentBackup.reminder.body', { count: onlyInPathkeep })}
        </span>
        <button
          type="button"
          onClick={() =>
            void supportClient.openExternalUrl(DOCS_URL).catch((error) =>
              toast.error(t('agentBackup.reminder.linkFailed'), {
                description: describeError(error, 'open_external_url'),
              }),
            )
          }
          className="flex w-fit items-center gap-1 text-brand hover:underline"
        >
          {t('agentBackup.reminder.docs')}
          <ExternalLink aria-hidden className="size-3" />
        </button>
      </div>
      <Button
        size="sm"
        variant="ghost"
        aria-label={t('agentBackup.reminder.dismissLabel')}
        onClick={dismiss}
        className="shrink-0 text-muted-foreground"
      >
        {t('agentBackup.reminder.dismiss')}
      </Button>
    </section>
  )
}
