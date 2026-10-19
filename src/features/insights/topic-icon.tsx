/**
 * What a topic the user keeps coming back to is, at a glance: a page shows
 * its site's icon, a search the search icon. (The rows used coloured dots,
 * copied from the prototype; their colours meant nothing.)
 */
import { FileText, Search } from 'lucide-react'
import { SiteIcon } from '@/features/history/site-icon'
import type { ReopenedInvestigation } from '@/lib/core-intelligence/types'

export function TopicIcon({ topic }: { topic: ReopenedInvestigation }) {
  if (topic.anchorType === 'query_family')
    return (
      <span className="flex size-[18px] shrink-0 items-center justify-center text-muted-foreground">
        <Search className="size-3.5" aria-hidden />
      </span>
    )
  if (topic.site)
    return <SiteIcon domain={topic.site} className="size-[18px] text-[10px]" />
  return (
    <span className="flex size-[18px] shrink-0 items-center justify-center text-muted-foreground">
      <FileText className="size-3.5" aria-hidden />
    </span>
  )
}
