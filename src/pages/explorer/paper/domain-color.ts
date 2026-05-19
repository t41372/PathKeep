/**
 * Deterministic domain → color mapping for contact-sheet frames.
 *
 * The design uses a small palette of muted accents (clay, slate, sage,
 * dusty rose, ochre, sand) so domain blocks stay readable on cream paper.
 * Picking by stable hash keeps the same domain the same color between
 * renders without holding state.
 */

const PALETTE = [
  '#8b8470', // sand
  '#9a8475', // clay
  '#7a8b8a', // mist
  '#a18573', // brick
  '#8c9788', // sage
  '#9c8593', // mauve
  '#a3937a', // ochre
  '#7e8a93', // slate
  '#a08680', // dust
  '#8d9080', // moss
]

export function hashString(value: string): number {
  let hash = 0
  for (let i = 0; i < value.length; i += 1) {
    hash = (hash * 31 + value.charCodeAt(i)) | 0
  }
  return Math.abs(hash)
}

export function getDomainColor(domain: string): string {
  if (!domain) return PALETTE[0]
  const stripped = domain.replace(/^www\./i, '').toLowerCase()
  return PALETTE[hashString(stripped) % PALETTE.length]
}

export function getDomainAbbr(domain: string): string {
  if (!domain) return '·'
  const stripped = domain.replace(/^www\./i, '').toLowerCase()
  const segments = stripped.split('.').filter(Boolean)
  if (segments.length === 0) return '·'
  if (segments.length === 1) return segments[0].slice(0, 3).toUpperCase()
  const head = segments[0]
  return head.slice(0, 3).toUpperCase()
}
