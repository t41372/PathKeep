/**
 * Site badge: a stored favicon when we have one, otherwise the first letter
 * of the domain on a color derived from the domain (as in the prototype).
 * The letter is drawn by CSS, not written as text, so it is not part of the
 * row it sits in when that is read or copied.
 */
import { cn } from '@/lib/cn'

const hues = [252, 150, 30, 300, 200, 90, 340, 0]

const host = (domain: string) => domain.replace(/^www\./, '')

/** By host without `www.`, so a site keeps its colour wherever it is listed. */
function domainColor(domain: string) {
  let hash = 0
  for (const char of host(domain)) hash = (hash * 31 + char.charCodeAt(0)) % 997
  return `oklch(0.62 0.13 ${hues[hash % hues.length]})`
}

function domainInitial(domain: string) {
  return (host(domain)[0] ?? '?').toUpperCase()
}

export function Favicon({
  domain,
  src,
  className,
}: {
  domain: string
  src?: string | null
  className?: string
}) {
  if (src) {
    return (
      <img
        src={src}
        alt=""
        className={cn(
          'size-[18px] shrink-0 rounded-[5px] object-contain',
          className,
        )}
      />
    )
  }
  return (
    <span
      aria-hidden
      data-initial={domainInitial(domain)}
      className={cn(
        'flex size-[18px] shrink-0 items-center justify-center rounded-[5px] text-[10px] font-semibold text-white before:content-[attr(data-initial)]',
        className,
      )}
      style={{ background: domainColor(domain) }}
    />
  )
}
