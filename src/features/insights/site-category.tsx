/**
 * A site's category, on its Insights page: the one PathKeep's lists give it,
 * or one the user picks (`deterministic-intelligence.md` §5.2: the user's
 * choice comes first). Only the site's category: its pages keep their kinds,
 * and the parts of the site the lists sort on their own (`mail.google.com` in
 * `google.com`) keep theirs, which the control lists. Insights are built
 * again with it, sorting only this site's pages in; until then the control
 * says Insights are being updated.
 *
 * Responsible for: showing and changing the category. Not responsible for:
 * what a category does to the cards (the backend), or listing the sites no
 * list knows (`unclassified-sites.tsx`).
 */
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import type { DomainDeepDive, SitePart } from '@/lib/core-intelligence/types'
import { describeError } from '@/lib/errors'
import { useI18n } from '@/lib/i18n'
import { category, SITE_CATEGORIES, UNCLASSIFIED } from './categories'
import { useSetSiteCategory } from './drill-queries'

/** The choice that gives the site back to PathKeep's lists. */
const LISTS = 'lists'

export function SiteCategory({
  domain,
  site,
}: {
  domain: string
  site: DomainDeepDive | undefined
}) {
  const { t } = useI18n()
  const change = useSetSiteCategory(domain)
  if (!site) return null
  const listed = t(
    `insights.mix.categories.${category(site.listCategory ?? UNCLASSIFIED)}`,
  )
  const updating = change.isPending || site.categoryPending
  return (
    <div className="flex items-center gap-2">
      {updating && (
        <span className="text-xs text-muted-foreground">
          {t('insights.site.category.updating')}
        </span>
      )}
      {site.listedParts && site.listedParts.length > 0 && (
        <ListedParts domain={domain} parts={site.listedParts} />
      )}
      <Select
        value={site.userCategory ?? LISTS}
        disabled={change.isPending}
        onValueChange={(next) =>
          change.mutate(next === LISTS ? null : next, {
            onError: (error) =>
              toast.error(t('insights.site.category.failed'), {
                description: describeError(error, 'set_site_category'),
              }),
          })
        }
      >
        <SelectTrigger
          size="sm"
          aria-label={t('insights.site.category.label')}
          className="min-w-[150px] bg-card text-[13px]"
        >
          <SelectValue />
        </SelectTrigger>
        <SelectContent align="end">
          <SelectItem value={LISTS}>
            {t('insights.site.category.lists', { category: listed })}
          </SelectItem>
          {SITE_CATEGORIES.map((name) => (
            <SelectItem key={name} value={name}>
              {t(`insights.mix.categories.${name}`)}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  )
}

/** The parts of the site the lists sort on their own, with their kinds. */
function ListedParts({ domain, parts }: { domain: string; parts: SitePart[] }) {
  const { t } = useI18n()
  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button
          variant="ghost"
          size="sm"
          className="text-xs text-muted-foreground"
        >
          {t('insights.site.category.parts', { count: parts.length })}
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="flex w-80 flex-col gap-2">
        <p className="text-[13px] leading-snug">
          {t('insights.site.category.partsExplain', { site: domain })}
        </p>
        <ul className="flex flex-col gap-1 text-[13px]">
          {parts.map((part) => (
            <li key={part.host} className="flex items-center gap-3">
              <span className="flex-1 truncate font-mono text-xs">
                {part.host}
              </span>
              <span className="text-muted-foreground">
                {t(`insights.mix.categories.${category(part.category)}`)}
              </span>
            </li>
          ))}
        </ul>
      </PopoverContent>
    </Popover>
  )
}
