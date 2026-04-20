/**
 * Shared PME tab bar used by routes that expose Preview / Manual / Execute /
 * Verify without re-implementing the same tablist shell.
 */

import type { ReactNode } from 'react'

export interface PmeTabOption<T extends string> {
  key: T
  label: ReactNode
}

export function PmeTabBar<T extends string>({
  activeTab,
  onChange,
  tabs,
}: {
  activeTab: T
  onChange: (tab: T) => void
  tabs: PmeTabOption<T>[]
}) {
  return (
    <div className="pme-tabs">
      {tabs.map((tab) => (
        <button
          key={tab.key}
          aria-pressed={activeTab === tab.key}
          className={`pme-tab ${activeTab === tab.key ? 'active' : ''}`}
          type="button"
          onClick={() => onChange(tab.key)}
        >
          {tab.label}
        </button>
      ))}
    </div>
  )
}
