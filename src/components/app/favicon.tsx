/**
 * Site badge: a stored favicon when we have one, otherwise the first letter
 * of the domain on a color derived from the domain (as in the prototype).
 */
import { cn } from '@/lib/cn'

const hues = [252, 150, 30, 300, 200, 90, 340, 0]

function domainColor(domain: string) {
  let hash = 0
  for (const char of domain) hash = (hash * 31 + char.charCodeAt(0)) % 997
  return `oklch(0.62 0.13 ${hues[hash % hues.length]})`
}

function domainInitial(domain: string) {
  const host = domain.replace(/^www\./, '')
  return (host[0] ?? '?').toUpperCase()
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
      className={cn(
        'flex size-[18px] shrink-0 items-center justify-center rounded-[5px] text-[10px] font-semibold text-white',
        className,
      )}
      style={{ background: domainColor(domain) }}
    >
      {domainInitial(domain)}
    </span>
  )
}
