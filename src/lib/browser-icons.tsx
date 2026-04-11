/**
 * This module resolves browser names into the iconography used across onboarding, dashboard, and scoped review surfaces.
 *
 * Why this file exists:
 * - Files in `src/lib/` are where UI policy becomes testable without inflating every route component.
 * - If you are trying to understand a front-end contract quickly, these helpers usually explain the reusable part of the story.
 *
 * Main declarations:
 * - `supportedBrowsers`
 * - `browserIconKeyForName`
 * - `BrowserIcon`
 *
 * Source-of-truth notes:
 * - Keep helper behavior aligned with the shipping design, feature, and architecture docs rather than local route assumptions.
 * - Avoid burying user-visible copy or route-only workflow rules here unless the helper truly owns that cross-cutting contract.
 */

import type { ReactNode } from 'react'

/* eslint-disable react-refresh/only-export-components */
/**
 * Defines the type-level contract for browser icon key.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
type BrowserIconKey =
  | 'arc'
  | 'brave'
  | 'chrome'
  | 'chromium'
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

export const supportedBrowsers = [
  { key: 'chrome', name: 'Google Chrome' },
  { key: 'chromium', name: 'Chromium' },
  { key: 'edge', name: 'Microsoft Edge' },
  { key: 'edge-dev', name: 'Microsoft Edge Dev' },
  { key: 'brave', name: 'Brave' },
  { key: 'vivaldi', name: 'Vivaldi' },
  { key: 'arc', name: 'Arc' },
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

/**
 * Explains how browser icon key for name works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function browserIconKeyForName(browserName: string): BrowserIconKey {
  return browserIconKeyByName.get(browserName) ?? 'generic'
}

/**
 * Defines the type-level contract for browser icon props.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
type BrowserIconProps = {
  browserName: string
  className?: string
  decorative?: boolean
  title?: string
}

/**
 * Explains how browser icon works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
export function BrowserIcon({
  browserName,
  className,
  decorative = false,
  title,
}: BrowserIconProps) {
  const key = browserIconKeyForName(browserName)
  const accessibilityProps = decorative
    ? { 'aria-hidden': true }
    : {
        role: 'img' as const,
        'aria-label': title ?? `${browserName} icon`,
      }

  return (
    <svg
      viewBox="0 0 24 24"
      className={['browserIcon', className].filter(Boolean).join(' ')}
      {...accessibilityProps}
    >
      {renderBrowserIcon(key)}
    </svg>
  )
}

/**
 * Explains how render browser icon works.
 *
 * This helper should stay small, explicit, and easy to test because multiple routes rely on it as a shared contract.
 */
function renderBrowserIcon(key: BrowserIconKey): ReactNode {
  switch (key) {
    case 'chrome':
      return (
        <>
          <path d="M12 2a10 10 0 0 1 8.66 5H12Z" fill="#db4437" />
          <path d="M20.66 7A10 10 0 0 1 11.65 22l4.42-7.67Z" fill="#0f9d58" />
          <path d="M11.65 22A10 10 0 0 1 12 2l4.83 8.38Z" fill="#f4b400" />
          <circle
            cx="12"
            cy="12"
            r="4.6"
            fill="#4285f4"
            stroke="#fff"
            strokeWidth="1.1"
          />
        </>
      )
    case 'chromium':
      return (
        <>
          <circle cx="12" cy="12" r="9.5" fill="#174ea6" />
          <circle cx="12" cy="12" r="6.3" fill="#5b9cf6" />
          <circle cx="12" cy="12" r="3.5" fill="#dce9ff" />
        </>
      )
    case 'edge':
      return (
        <>
          <path
            d="M20 15.5A8 8 0 0 1 4.6 18c.8-2.9 3.5-5.2 7.3-5.2 2.2 0 4.2.8 5.9 2.7-.6-3.1-2.9-5.7-6.6-5.7-3.6 0-6.1 2.2-7.1 4.6A8 8 0 0 1 20 15.5Z"
            fill="#0ea5e9"
          />
          <path
            d="M19.7 15.4c-1.7 2.6-4.4 4.1-7.5 4.1-2.7 0-5-1-6.9-2.8.8 0 1.5-.1 2.3-.1 5.3 0 8.8-2.1 10.8-6.7 1.1 1.3 1.6 3 1.3 5.5Z"
            fill="#10b981"
          />
        </>
      )
    case 'edge-dev':
      return (
        <>
          {renderBrowserIcon('edge')}
          <circle cx="18.7" cy="5.3" r="2.4" fill="#7c3aed" />
          <path
            d="M17.4 5.3h2.6M18.7 4v2.6"
            stroke="#fff"
            strokeWidth="1"
            strokeLinecap="round"
          />
        </>
      )
    case 'brave':
      return (
        <>
          <path
            d="M12 2.6 18.5 5l1.2 5.1-2.2 8L12 21.4 6.5 18.1l-2.2-8L5.5 5Z"
            fill="#f97316"
          />
          <text
            x="12"
            y="15"
            textAnchor="middle"
            fontSize="8.5"
            fontWeight="800"
            fill="#fff"
          >
            B
          </text>
        </>
      )
    case 'vivaldi':
      return (
        <>
          <circle cx="12" cy="12" r="9.5" fill="#ef4444" />
          <path
            d="M8 7.2 12 16l4-8.8"
            fill="none"
            stroke="#fff"
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth="2.2"
          />
        </>
      )
    case 'arc':
      return (
        <>
          <rect
            x="2.5"
            y="2.5"
            width="19"
            height="19"
            rx="5.2"
            fill="#111827"
          />
          <path
            d="M7 15.8a5.4 5.4 0 0 1 10 0"
            fill="none"
            stroke="#fff"
            strokeLinecap="round"
            strokeWidth="2"
          />
          <path
            d="M9.2 13.8a3.1 3.1 0 0 1 5.6 0"
            fill="none"
            stroke="#9ca3af"
            strokeLinecap="round"
            strokeWidth="1.6"
          />
        </>
      )
    case 'opera':
      return (
        <>
          <circle cx="12" cy="12" r="9.3" fill="#ef4444" />
          <ellipse cx="12" cy="12" rx="4.5" ry="6.7" fill="#fff" />
          <ellipse cx="12" cy="12" rx="2.8" ry="5" fill="#ef4444" />
        </>
      )
    case 'opera-gx':
      return (
        <>
          <circle cx="12" cy="12" r="9.3" fill="#ec4899" />
          <ellipse cx="12" cy="12" rx="4.6" ry="6.8" fill="#111827" />
          <ellipse cx="12" cy="12" rx="3.2" ry="5.2" fill="#ec4899" />
          <path
            d="M8.6 16.5 15.9 7.5"
            stroke="#fff"
            strokeLinecap="round"
            strokeWidth="1.3"
          />
        </>
      )
    case 'firefox':
      return (
        <>
          <circle cx="12" cy="12" r="9.5" fill="#7c3aed" />
          <path
            d="M18 8.5c-1-2.1-3-3.7-5.8-3.7-3.7 0-6.6 3-6.6 6.7 0 3.8 3 6.7 6.9 6.7 3.5 0 6.1-2.4 6.4-5.6-.7.6-1.7 1-2.8 1-.4-2.1-1.4-3.9-3-5.2.1-.5.6-1.1 1.3-1.6.8-.5 1.8-.8 3.6-.3Z"
            fill="#f59e0b"
          />
          <path
            d="M9 17.7c1.1-.3 2-.9 2.8-1.8-1.9-.3-3.3-1.7-3.3-3.6 0-1.5 1-2.8 2.3-3.4-.2 3.1 1.1 5.5 4.2 6.9-.6 1.2-2.2 2.1-4 2.1-.7 0-1.3-.1-2-.2Z"
            fill="#fb7185"
          />
        </>
      )
    case 'librewolf':
      return (
        <>
          <circle cx="12" cy="12" r="9.5" fill="#2563eb" />
          <path
            d="M6.8 15.7c2-.8 3.6-2.5 4.5-4.7 1.5 1.8 3.4 2.9 5.9 3.3-1 2-3.1 3.4-5.6 3.4-1.8 0-3.5-.7-4.8-2Z"
            fill="#e5f1ff"
          />
          <path d="M9.2 6.5 8 9.1l2.5-.7Z" fill="#c7d2fe" />
        </>
      )
    case 'floorp':
      return (
        <>
          <rect
            x="2.5"
            y="2.5"
            width="19"
            height="19"
            rx="5.5"
            fill="#0f172a"
          />
          <path
            d="M6.2 9.2h11.6M6.2 12h8.8M6.2 14.8h10.2"
            stroke="#38bdf8"
            strokeLinecap="round"
            strokeWidth="2"
          />
        </>
      )
    case 'waterfox':
      return (
        <>
          <circle cx="12" cy="12" r="9.5" fill="#0f766e" />
          <path
            d="M6.6 7.4 8.5 16.5l3.5-5 3.5 5 1.9-9.1"
            fill="none"
            stroke="#fff"
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth="2"
          />
        </>
      )
    case 'safari':
      return (
        <>
          <circle cx="12" cy="12" r="9.5" fill="#e0f2fe" />
          <circle
            cx="12"
            cy="12"
            r="7.2"
            fill="#f8fafc"
            stroke="#0ea5e9"
            strokeWidth="1.4"
          />
          <path d="M12 6.2 14.8 12 12 17.8 9.2 12Z" fill="#ef4444" />
          <path
            d="M12 17.8 9.2 12 12 6.2 14.8 12Z"
            fill="#0f172a"
            opacity="0.72"
          />
        </>
      )
    case 'generic':
      return (
        <>
          <circle cx="12" cy="12" r="9.5" fill="#334155" />
          <path
            d="M4.5 12h15M12 4.5a11 11 0 0 1 0 15M12 4.5a11 11 0 0 0 0 15"
            fill="none"
            stroke="#e2e8f0"
            strokeLinecap="round"
            strokeWidth="1.2"
          />
        </>
      )
  }
}
