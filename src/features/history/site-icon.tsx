/**
 * A site's stored favicon, with the letter badge until (or unless) one is
 * found. With a visit it is looked up through the visit; without one, as on
 * Insights where a row is a site, the site's own icon is asked for.
 */
import { useEffect } from 'react'
import { Favicon } from '@/components/app/favicon'
import type { HistoryFaviconLookupEntry } from '@/lib/types'
import { requestFavicon, requestSiteFavicon, useFavicon } from './favicon-store'

export function SiteIcon({
  domain,
  lookup,
  className,
}: {
  domain: string
  /** A visit to look the icon up through; without one, the site's own icon. */
  lookup?: HistoryFaviconLookupEntry
  className?: string
}) {
  const src = useFavicon(domain)
  useEffect(() => {
    if (lookup) requestFavicon(domain, lookup)
    else requestSiteFavicon(domain)
  }, [domain, lookup])
  return <Favicon domain={domain} src={src} className={className} />
}
