/**
 * Agent conversations' line in the space switcher's menu: how many
 * conversations the archive holds and how many of them are only in
 * PathKeep now (their files are gone from the tool's folder).
 *
 * Responsibilities: those two numbers, from the same committed facts as the
 * backup page, and a quiet placeholder while they load. Mounted only while
 * the menu is open. A store that will not open shows a dash, not zero.
 */
import { Skeleton } from '@/components/ui/skeleton'
import { useI18n } from '@/lib/i18n'
import { useAgentOverview, wireNumber } from '@/lib/queries/agent-backup'

export default function AgentStatus() {
  const { t } = useI18n()
  const overview = useAgentOverview('space')
  if (overview.isPending && overview.fetchStatus !== 'idle') {
    return <Skeleton aria-hidden className="my-[3px] h-3 w-32" />
  }
  const data = overview.data
  return (
    <>
      {t('spaces.switcher.statusLine', {
        first: data
          ? t('spaces.agent.conversations', { count: wireNumber(data.roots) })
          : '—',
        second: data
          ? t('spaces.agent.onlyInPathkeep', {
              count: wireNumber(data.onlyInPathkeep),
            })
          : '—',
      })}
    </>
  )
}
