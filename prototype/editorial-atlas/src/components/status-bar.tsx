import { CheckCircle } from 'lucide-react'
import { archiveStats } from '@/lib/mock-data'

export function StatusBar() {
  const formatDate = (date: Date) => {
    return date.toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    }) + ' at ' + date.toLocaleTimeString('en-US', {
      hour: 'numeric',
      minute: '2-digit',
      hour12: true,
    })
  }

  return (
    <footer className="flex items-center justify-between px-4 py-2 border-t border-border bg-paper text-xs font-mono text-ink-tertiary">
      <div className="flex items-center gap-4">
        <span>{archiveStats.totalPages.toLocaleString()} pages</span>
        <span className="text-ink-faint">·</span>
        <span>{archiveStats.totalSize}</span>
        <span className="text-ink-faint">·</span>
        <span>{archiveStats.syncStatus}</span>
      </div>
      <div className="flex items-center gap-1.5">
        <span>Indexed on {formatDate(archiveStats.lastIndexed)}</span>
        <CheckCircle className="w-3.5 h-3.5 text-green-600" />
      </div>
    </footer>
  )
}
