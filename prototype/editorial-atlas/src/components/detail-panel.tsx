import { 
  ExternalLink, 
  Check, 
  FolderOpen, 
  Plus,
  ChevronRight
} from 'lucide-react'
import { cn } from '@/lib/utils'
import type { HistoryEntry } from '@/lib/mock-data'

// Simple domain-based favicon colors (same as timeline-entry)
function getDomainColor(domain: string): string {
  const colors: Record<string, string> = {
    'aeon.co': 'bg-amber-700',
    'nesslabs.com': 'bg-emerald-600',
    'github.com': 'bg-gray-800',
    'stratechery.com': 'bg-green-700',
    'zhihu.com': 'bg-blue-600',
    'sspai.com': 'bg-red-500',
    'note.com': 'bg-green-500',
    'zenn.dev': 'bg-blue-500',
    'velog.io': 'bg-teal-500',
    'youtube.com': 'bg-red-600',
    'docs.rs': 'bg-orange-600',
  }
  return colors[domain] || 'bg-ink-tertiary'
}

interface DetailPanelProps {
  entry: HistoryEntry | null
}

export function DetailPanel({ entry }: DetailPanelProps) {
  if (!entry) {
    return (
      <aside className="w-72 flex-shrink-0 border-l border-border bg-paper p-4">
        <div className="h-full flex items-center justify-center">
          <p className="text-sm text-ink-tertiary italic text-center">
            Select an entry to view details
          </p>
        </div>
      </aside>
    )
  }

  const visitHistory = [
    { date: 'Apr 24', count: 0 },
    { date: 'May 4', count: 1 },
    { date: 'May 14', count: 1 },
    { date: 'May 24', count: 1 },
  ]

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
    <aside className="w-72 flex-shrink-0 border-l border-border bg-paper overflow-y-auto">
      <div className="p-4 space-y-5">
        {/* Header - Domain & URL */}
        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <div className={cn(
              'w-8 h-8 rounded flex items-center justify-center text-white text-sm font-semibold',
              getDomainColor(entry.domain)
            )}>
              {entry.domain.charAt(0).toUpperCase()}
            </div>
            <span className="font-serif text-lg font-medium text-ink">
              {entry.domain}
            </span>
          </div>
          <a 
            href={entry.url}
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center gap-1 text-xs text-oxblood hover:underline break-all"
          >
            <span className="truncate">{entry.url}</span>
            <ExternalLink className="w-3 h-3 flex-shrink-0" />
          </a>
        </div>

        {/* Summary */}
        <div className="space-y-1.5">
          <h4 className="text-xs font-medium text-ink-tertiary uppercase tracking-wider">
            Summary
          </h4>
          <p className="text-sm text-ink-secondary leading-relaxed">
            {entry.note || 'An exploration of slower, more intentional ways of working in a world that glorifies speed.'}
          </p>
        </div>

        {/* Metadata */}
        <div className="space-y-2">
          <h4 className="text-xs font-medium text-ink-tertiary uppercase tracking-wider">
            Metadata
          </h4>
          <div className="space-y-1.5">
            <MetadataRow label="First Visited" value={formatDate(entry.timestamp)} />
            <MetadataRow label="Last Visited" value={formatDate(entry.timestamp)} />
            <MetadataRow label="Visit Count" value={entry.visitCount.toString()} />
            {entry.timeSpent && (
              <MetadataRow label="Time Spent" value={entry.timeSpent} />
            )}
            <MetadataRow label="Source" value={entry.source} />
            <MetadataRow label="Page Type" value={entry.type.charAt(0).toUpperCase() + entry.type.slice(1)} />
          </div>
        </div>

        {/* Tags */}
        <div className="space-y-2">
          <h4 className="text-xs font-medium text-ink-tertiary uppercase tracking-wider">
            Tags
          </h4>
          <div className="flex flex-wrap gap-1.5">
            {['productivity', 'focus', 'writing', 'mindset'].map((tag) => (
              <span key={tag} className="tag">{tag}</span>
            ))}
            <button className="tag hover:bg-paper-hover">
              <Plus className="w-3 h-3" />
            </button>
          </div>
        </div>

        {/* Connections */}
        <div className="space-y-2">
          <h4 className="text-xs font-medium text-ink-tertiary uppercase tracking-wider">
            Connections
          </h4>
          <div className="space-y-1.5">
            <ConnectionRow label="In Collection" value="Writing Research" />
            <ConnectionRow label="Related" value="3 pages" />
          </div>
        </div>

        {/* Local-First Status */}
        <div className="p-3 bg-paper-card border border-border rounded space-y-1.5">
          <div className="flex items-center gap-2">
            <div className="w-4 h-4 rounded-full bg-green-100 flex items-center justify-center">
              <Check className="w-2.5 h-2.5 text-green-700" />
            </div>
            <span className="text-sm font-medium text-ink">Fully Local</span>
          </div>
          <p className="text-xs text-ink-secondary">
            This page and all associated data are stored securely on this device.
          </p>
          <button className="flex items-center gap-1 text-xs text-oxblood hover:underline">
            <FolderOpen className="w-3 h-3" />
            View Data Directory
            <ChevronRight className="w-3 h-3" />
          </button>
        </div>

        {/* Visit History Chart */}
        <div className="space-y-2">
          <h4 className="text-xs font-medium text-ink-tertiary uppercase tracking-wider">
            Visit History
          </h4>
          <div className="h-16 flex items-end justify-between gap-6 px-2">
            {visitHistory.map((item, i) => (
              <div key={i} className="flex flex-col items-center gap-1">
                <div className="flex flex-col-reverse gap-0.5">
                  {[0, 1].map((level) => (
                    <div 
                      key={level}
                      className={cn(
                        'w-3 h-4 rounded-sm',
                        item.count > level ? 'bg-oxblood' : 'bg-transparent'
                      )}
                    />
                  ))}
                </div>
                <span className="text-2xs text-ink-tertiary">{item.date}</span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </aside>
  )
}

function MetadataRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between text-sm">
      <span className="text-ink-tertiary">{label}</span>
      <span className="text-ink">{value}</span>
    </div>
  )
}

function ConnectionRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between text-sm">
      <span className="text-ink-tertiary">{label}</span>
      <span className="text-oxblood hover:underline cursor-pointer">{value}</span>
    </div>
  )
}
