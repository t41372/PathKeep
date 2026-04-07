import pathkeepMarkUrl from '../assets/pathkeep-mark.svg'

interface BrandMarkProps {
  alt?: string
  className?: string
}

export function BrandMark({
  alt = 'PathKeep',
  className = '',
}: BrandMarkProps) {
  return (
    <img
      alt={alt}
      className={className ? `brand-mark ${className}` : 'brand-mark'}
      src={pathkeepMarkUrl}
    />
  )
}
