/**
 * Link previews: the page's own og:image, as a card at the top of the detail
 * panel and as a small thumbnail in the Starred and Sites rows.
 *
 * Responsible for: the fixed-size boxes and every state they can be in
 * (looking up, downloading, shown, none, blocked, failed, turned off).
 * Not responsible for: loading or caching images (`og-image-store.ts`).
 *
 * Both boxes have a fixed size from the first paint, so nothing around them
 * moves when an image arrives or turns out not to exist. Rows only read what
 * the archive already holds; only the detail panel downloads a missing one.
 */
import { ImageOff } from 'lucide-react'
import { useEffect, useState } from 'react'
import { Link } from 'react-router-dom'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import type { HistoryFaviconLookupEntry } from '@/lib/types'
import {
  fetchOgImage,
  markOgImageShown,
  useOgImage,
  usePreviewFetching,
  wasFetched,
  type OgImageState,
} from './og-image-store'
import { SiteIcon } from './site-icon'

type PreviewState =
  | 'loading'
  | 'fetching'
  | 'ok'
  | 'missing'
  | 'blocked'
  | 'https'
  | 'failed'
  | 'off'

function stateOf(
  url: string,
  image: OgImageState,
  canFetch: boolean,
  broken: boolean,
): PreviewState {
  if (image === undefined) return 'loading'
  if (image === 'fetching') return 'fetching'
  if (image.status === 'ok' && image.dataUrl && !broken) return 'ok'
  if (image.status === 'missing') return 'missing'
  if (image.status === 'blocked') return 'blocked'
  // Plain-http pages are never fetched; the backend records them as parse errors.
  if (url.startsWith('http://')) return 'https'
  if (image.status === 'pending') {
    if (!canFetch) return 'off'
    return wasFetched(url) ? 'failed' : 'loading'
  }
  // http_error, parse_error, too_large, unsupported_mime, or a broken image.
  return 'failed'
}

/** The fade-in image both boxes share; reports itself shown once it has decoded. */
function PreviewImage({
  url,
  src,
  alt,
  onBroken,
}: {
  url: string
  src: string
  alt: string
  onBroken: () => void
}) {
  const [loaded, setLoaded] = useState(false)
  return (
    <img
      src={src}
      alt={alt}
      decoding="async"
      draggable={false}
      onLoad={() => {
        setLoaded(true)
        markOgImageShown(url)
      }}
      onError={onBroken}
      className={cn(
        'absolute inset-0 size-full object-cover transition-opacity duration-300 motion-reduce:transition-none',
        loaded ? 'opacity-100' : 'opacity-0',
      )}
    />
  )
}

/** The detail panel's preview card: 1.91 : 1, the shape og:image is made for. */
export function LinkPreview({
  url,
  domain,
  title,
}: {
  url: string
  domain: string
  title: string
}) {
  const { t } = useI18n()
  const canFetch = usePreviewFetching()
  const image = useOgImage(url)
  const [broken, setBroken] = useState(false)
  const state = stateOf(url, image, canFetch, broken)

  // A page nobody has tried yet is downloaded once, when its panel opens.
  const pending =
    image !== undefined && image !== 'fetching' && image.status === 'pending'
  useEffect(() => {
    if (pending && canFetch && !url.startsWith('http://'))
      void fetchOgImage(url)
  }, [pending, canFetch, url])

  const caption = (() => {
    switch (state) {
      case 'fetching':
        return t('historyPage.preview.fetching', { domain })
      case 'missing':
        return t('historyPage.preview.missing')
      case 'blocked':
        return t('historyPage.preview.blocked')
      case 'https':
        return t('historyPage.preview.httpsOnly')
      case 'failed':
        return t('historyPage.preview.failed')
      case 'off':
        return t('historyPage.preview.off')
      default:
        return null
    }
  })()

  return (
    <figure
      aria-label={t('historyPage.preview.label')}
      aria-busy={state === 'loading' || state === 'fetching'}
      data-preview-state={state}
      className="relative aspect-[1.91/1] w-full shrink-0 overflow-hidden rounded-[10px] border bg-muted"
    >
      {state === 'ok' && image && image !== 'fetching' && image.dataUrl ? (
        <PreviewImage
          url={url}
          src={image.dataUrl}
          alt={t('historyPage.preview.alt', { title })}
          onBroken={() => setBroken(true)}
        />
      ) : (
        <div
          className={cn(
            'absolute inset-0 flex flex-col items-center justify-center gap-2 px-4 pb-6',
            (state === 'loading' || state === 'fetching') &&
              'animate-pulse motion-reduce:animate-none',
          )}
        >
          <SiteIcon domain={domain} className="size-8 rounded-[8px] text-sm" />
          <span className="max-w-full truncate text-xs text-muted-foreground">
            {domain}
          </span>
        </div>
      )}
      {caption && (
        <figcaption className="absolute inset-x-0 bottom-0 flex items-center gap-1.5 bg-gradient-to-t from-background/80 to-transparent px-3 pt-4 pb-2 text-[11px] text-muted-foreground">
          {state !== 'fetching' && (
            <ImageOff className="size-3 shrink-0" aria-hidden />
          )}
          <span className="min-w-0 flex-1 truncate">{caption}</span>
          {state === 'failed' && canFetch && (
            <Button
              variant="ghost"
              size="xs"
              className="-my-1 h-5 px-1.5 text-[11px]"
              onClick={() => {
                setBroken(false)
                void fetchOgImage(url, { retry: true })
              }}
            >
              {t('historyPage.preview.retry')}
            </Button>
          )}
          {state === 'off' && (
            <Link
              to="/settings/general"
              className="shrink-0 rounded text-foreground underline-offset-2 outline-none hover:underline focus-visible:ring-2 focus-visible:ring-ring/50"
            >
              {t('historyPage.preview.turnOn')}
            </Link>
          )}
        </figcaption>
      )}
    </figure>
  )
}

/**
 * A row thumbnail: the stored preview when there is one, otherwise the site
 * icon in the same box. Reads only; it never starts a download.
 */
export function PreviewThumb({
  url,
  domain,
  lookup,
  className,
}: {
  /** The page whose preview to show; null shows the site icon only. */
  url: string | null
  domain: string
  /** A visit to look the favicon up through. */
  lookup?: HistoryFaviconLookupEntry
  className?: string
}) {
  const image = useOgImage(url)
  const [broken, setBroken] = useState(false)
  const src =
    url && image && image !== 'fetching' && image.status === 'ok' && !broken
      ? image.dataUrl
      : null

  return (
    <span
      aria-hidden
      data-preview-state={src ? 'ok' : image === undefined ? 'loading' : 'none'}
      className={cn(
        'relative flex h-9 w-16 shrink-0 items-center justify-center overflow-hidden rounded-[7px] border bg-muted',
        className,
      )}
    >
      <SiteIcon
        domain={domain}
        lookup={lookup}
        className="size-5 rounded-[5px] text-[10px]"
      />
      {url && src && (
        <PreviewImage
          url={url}
          src={src}
          alt=""
          onBroken={() => setBroken(true)}
        />
      )}
    </span>
  )
}
