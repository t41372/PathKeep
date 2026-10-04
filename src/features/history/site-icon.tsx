/** A site's stored favicon, with the letter badge until (or unless) one is found. */
import { useEffect } from 'react'
import { Favicon } from '@/components/app/favicon'
import type { HistoryFaviconLookupEntry } from '@/lib/types'
import { requestFavicon, useFavicon } from './favicon-store'

export function SiteIcon({
  domain,
  lookup,
  className,
}: {
  domain: string
  /** A visit to look the icon up through; without one only cached icons show. */
  lookup?: HistoryFaviconLookupEntry
  className?: string
}) {
  const src = useFavicon(domain)
  useEffect(() => {
    if (lookup) requestFavicon(domain, lookup)
  }, [domain, lookup])
  return <Favicon domain={domain} src={src} className={className} />
}
