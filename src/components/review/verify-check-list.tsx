/**
 * Shared verify-result rows for routes that need to leave behind visible
 * outcome signals after preview/manual/execute flows complete.
 */

import type { ReactNode } from 'react'
import { ReviewSection } from './review-surface'

export interface VerifyCheckItem {
  body?: ReactNode
  key: string
  label: ReactNode
  status: ReactNode
}

export function VerifyCheckList({ items }: { items: VerifyCheckItem[] }) {
  if (items.length === 0) {
    return null
  }

  return (
    <div className="settings-result-list">
      {items.map((item) => (
        <ReviewSection
          key={item.key}
          headerMeta={<span className="mono">{item.status}</span>}
          title={item.label}
        >
          {item.body ? <p>{item.body}</p> : null}
        </ReviewSection>
      ))}
    </div>
  )
}
