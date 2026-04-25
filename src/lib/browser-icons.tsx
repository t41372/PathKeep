/**
 * @file browser-icons.tsx
 * @description Resolves browser product names into bundled official browser icon assets.
 * @module browser-icons
 *
 * ## Responsibilities
 * - Keep the supported browser name/key list aligned with adapter support docs.
 * - Map browser names from backend discovery/import metadata to packaged icon assets.
 * - Render accessible or decorative browser icons through one shared component.
 *
 * ## Not responsible for
 * - Promoting an adapter into public-support copy.
 * - Discovering browser profiles or deciding parser capability.
 * - Rewriting vendor artwork.
 *
 * ## Dependencies
 * - Depends on `docs/architecture/browser-support-and-adapter-playbook.md` for support taxonomy.
 * - Depends on `src/assets/browser-icons/README.md` for asset provenance and refresh notes.
 * - Depends on Vite static asset imports so icons are bundled with the desktop app.
 *
 * ## Performance notes
 * - Icon lookup is a fixed-size map lookup; rendering uses small bundled assets and does not fetch remote media at runtime.
 */

import arcIconUrl from '../assets/browser-icons/arc.png'
import atlasIconUrl from '../assets/browser-icons/atlas.png'
import braveIconUrl from '../assets/browser-icons/brave.svg'
import chromeIconUrl from '../assets/browser-icons/chrome.png'
import chromiumIconUrl from '../assets/browser-icons/chromium.png'
import cometIconUrl from '../assets/browser-icons/comet.png'
import edgeDevIconUrl from '../assets/browser-icons/edge-dev.png'
import edgeIconUrl from '../assets/browser-icons/edge.png'
import firefoxIconUrl from '../assets/browser-icons/firefox.svg'
import floorpIconUrl from '../assets/browser-icons/floorp.svg'
import librewolfIconUrl from '../assets/browser-icons/librewolf.svg'
import operaGxIconUrl from '../assets/browser-icons/opera-gx.png'
import operaIconUrl from '../assets/browser-icons/opera.png'
import safariIconUrl from '../assets/browser-icons/safari.png'
import vivaldiIconUrl from '../assets/browser-icons/vivaldi.png'
import waterfoxIconUrl from '../assets/browser-icons/waterfox.png'

/* eslint-disable react-refresh/only-export-components */
/**
 * Defines the stable browser icon keys shared by support docs, tests, and UI surfaces.
 *
 * Unknown browser names intentionally fall back to `generic` so imported archives
 * from future adapters do not break route rendering.
 */
type BrowserIconKey =
  | 'arc'
  | 'atlas'
  | 'brave'
  | 'chrome'
  | 'chromium'
  | 'comet'
  | 'edge'
  | 'edge-dev'
  | 'firefox'
  | 'floorp'
  | 'generic'
  | 'librewolf'
  | 'opera'
  | 'opera-gx'
  | 'safari'
  | 'vivaldi'
  | 'waterfox'

type PackagedBrowserIconKey = Exclude<BrowserIconKey, 'generic'>

export const supportedBrowsers = [
  { key: 'chrome', name: 'Google Chrome' },
  { key: 'chromium', name: 'Chromium' },
  { key: 'edge', name: 'Microsoft Edge' },
  { key: 'edge-dev', name: 'Microsoft Edge Dev' },
  { key: 'brave', name: 'Brave' },
  { key: 'vivaldi', name: 'Vivaldi' },
  { key: 'arc', name: 'Arc' },
  { key: 'atlas', name: 'ChatGPT Atlas' },
  { key: 'comet', name: 'Perplexity Comet' },
  { key: 'opera', name: 'Opera' },
  { key: 'opera-gx', name: 'Opera GX' },
  { key: 'firefox', name: 'Firefox' },
  { key: 'librewolf', name: 'LibreWolf' },
  { key: 'floorp', name: 'Floorp' },
  { key: 'waterfox', name: 'Waterfox' },
  { key: 'safari', name: 'Safari' },
] as const satisfies ReadonlyArray<{ key: BrowserIconKey; name: string }>

const browserIconKeyByName = new Map<string, BrowserIconKey>(
  supportedBrowsers.map((browser) => [browser.name, browser.key]),
)

const browserIconAssetByKey = {
  arc: arcIconUrl,
  atlas: atlasIconUrl,
  brave: braveIconUrl,
  chrome: chromeIconUrl,
  chromium: chromiumIconUrl,
  comet: cometIconUrl,
  edge: edgeIconUrl,
  'edge-dev': edgeDevIconUrl,
  firefox: firefoxIconUrl,
  floorp: floorpIconUrl,
  librewolf: librewolfIconUrl,
  opera: operaIconUrl,
  'opera-gx': operaGxIconUrl,
  safari: safariIconUrl,
  vivaldi: vivaldiIconUrl,
  waterfox: waterfoxIconUrl,
} as const satisfies Record<PackagedBrowserIconKey, string>

/**
 * Resolves backend browser display names to stable icon keys.
 *
 * @param browserName Browser product display name from discovery/import metadata.
 * @returns The icon key if the browser is covered by the packaged support list, otherwise `generic`.
 */
export function browserIconKeyForName(browserName: string): BrowserIconKey {
  return browserIconKeyByName.get(browserName) ?? 'generic'
}

/**
 * Returns the bundled official icon asset for a supported browser name.
 *
 * The helper keeps tests and UI call sites focused on package coverage instead
 * of depending on Vite's hashed runtime asset URL details.
 *
 * @param browserName Browser product display name from discovery/import metadata.
 * @returns A packaged asset URL for supported browsers, or `null` for unknown browser names.
 */
export function browserIconAssetForName(browserName: string): string | null {
  const key = browserIconKeyForName(browserName)
  return key === 'generic' ? null : browserIconAssetByKey[key]
}

/**
 * Defines how call sites request browser icon rendering.
 *
 * `decorative` is used when adjacent text already names the browser, avoiding a
 * repeated accessible name in profile rows.
 */
type BrowserIconProps = {
  browserName: string
  className?: string
  decorative?: boolean
  title?: string
}

/**
 * Renders a bundled official browser icon while preserving a neutral unknown-browser fallback.
 *
 * @returns An image for supported browsers, or a small generic SVG when the browser name is unknown.
 */
export function BrowserIcon({
  browserName,
  className,
  decorative = false,
  title,
}: BrowserIconProps) {
  const assetUrl = browserIconAssetForName(browserName)
  const accessibilityProps = decorative
    ? { alt: '', 'aria-hidden': true }
    : {
        alt: title ?? `${browserName} icon`,
      }
  const iconClassName = ['browserIcon', className].filter(Boolean).join(' ')

  if (assetUrl) {
    return (
      <img
        className={iconClassName}
        src={assetUrl}
        decoding="async"
        draggable={false}
        {...accessibilityProps}
      />
    )
  }

  return (
    <svg
      viewBox="0 0 24 24"
      className={iconClassName}
      {...(decorative
        ? { 'aria-hidden': true }
        : {
            role: 'img' as const,
            'aria-label': title ?? `${browserName} icon`,
          })}
    >
      <circle cx="12" cy="12" r="9.5" fill="#334155" />
      <path
        d="M4.5 12h15M12 4.5a11 11 0 0 1 0 15M12 4.5a11 11 0 0 0 0 15"
        fill="none"
        stroke="#e2e8f0"
        strokeLinecap="round"
        strokeWidth="1.2"
      />
    </svg>
  )
}
