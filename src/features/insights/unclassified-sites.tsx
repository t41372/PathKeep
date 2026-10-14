/**
 * The sites PathKeep's lists do not know, opened from the "Not classified"
 * row of the "What kind of sites" card: most visits in the scope first, each
 * linking to its site page, where it can be given a category.
 * (`deterministic-intelligence.md` §6.3: the most visited unclassified sites
 * can be reviewed.)
 */
import { Link } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { SiteIcon } from '@/features/history/site-icon'
import { useI18n } from '@/lib/i18n'
import { UNCLASSIFIED } from './categories'
import { useSitesInCategory } from './drill-queries'
import { sitePath } from './links'
import { ListSkeleton } from './parts'
import type { Scope } from './range'
import { useInsightsFormat } from './zone'

export function UnclassifiedSites({
  scope,
  open,
  onOpenChange,
}: {
  scope: Scope
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const { t } = useI18n()
  const format = useInsightsFormat()
  const sites = useSitesInCategory(UNCLASSIFIED, scope, open)
  const items = sites.data?.pages.flatMap((page) => page.data.items) ?? []
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>{t('insights.mix.unclassified.title')}</DialogTitle>
          <DialogDescription>
            {t('insights.mix.unclassified.description')}
          </DialogDescription>
        </DialogHeader>
        {sites.isPending ? (
          <ListSkeleton rows={6} />
        ) : sites.isError ? (
          <p role="alert" className="text-[13px] text-destructive">
            {t('insights.mix.unclassified.failed')}
          </p>
        ) : items.length === 0 ? (
          <p className="text-[13px] text-muted-foreground">
            {t('insights.mix.unclassified.empty')}
          </p>
        ) : (
          <ul className="flex max-h-[50vh] flex-col gap-1 overflow-y-auto">
            {items.map((site) => (
              <li key={site.registrableDomain}>
                <Link
                  to={sitePath(site.registrableDomain, scope)}
                  onClick={() => onOpenChange(false)}
                  className="flex h-8 items-center gap-2.5 rounded-md px-2 text-[13px] hover:bg-muted hover:no-underline"
                >
                  <SiteIcon domain={site.registrableDomain} />
                  <span className="flex-1 truncate font-medium">
                    {site.registrableDomain}
                  </span>
                  <span className="font-mono text-xs text-muted-foreground">
                    {t('insights.mix.unclassified.visits', {
                      count: site.visitCount,
                      shown: format.number(site.visitCount),
                    })}
                  </span>
                </Link>
              </li>
            ))}
          </ul>
        )}
        {sites.hasNextPage && (
          <Button
            variant="outline"
            size="sm"
            disabled={sites.isFetchingNextPage}
            onClick={() => void sites.fetchNextPage()}
          >
            {t('insights.mix.unclassified.more')}
          </Button>
        )}
      </DialogContent>
    </Dialog>
  )
}
