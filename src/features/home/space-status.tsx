/**
 * Browsing's line in the space switcher's menu: visits in the archive and
 * browsers with at least one profile backed up, the same numbers Home's
 * "Saved" and "Sources" cards show.
 *
 * Responsibilities: those two numbers, from the reads Home already makes
 * (cached under the archive key, so a backup refreshes them), and a quiet
 * placeholder while they load. Mounted only while the menu is open.
 * Not responsible for: the menu or its row (`src/app/shell/space-switcher.tsx`).
 */
import { Skeleton } from '@/components/ui/skeleton'
import { useI18n } from '@/lib/i18n'
import { useDashboard } from '@/lib/queries/app'
import { useBrowserSummaries } from './home-data'

export default function BrowsingStatus() {
  const { t } = useI18n()
  const dashboard = useDashboard()
  const browsers = useBrowserSummaries()
  if (dashboard.isPending || browsers.loading) {
    return <Skeleton aria-hidden className="my-[3px] h-3 w-32" />
  }
  const backedUp = browsers.list.filter((browser) => browser.selected).length
  return (
    <>
      {t('spaces.switcher.statusLine', {
        // A failed read is unknown, not an empty archive.
        first: dashboard.data
          ? t('spaces.browsing.visits', { count: dashboard.data.totalVisits })
          : '—',
        second: t('spaces.browsing.browsers', { count: backedUp }),
      })}
    </>
  )
}
