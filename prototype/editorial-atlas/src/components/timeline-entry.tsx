import { Bookmark, FileEdit, Quote } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { HistoryEntry } from '@/lib/mock-data'

const typeLabels: Record<HistoryEntry['type'], string> = {
  article: 'Article',
  repository: 'Repository',
  video: 'Video',
  social: 'Social',
  docs: 'Docs',
  search: 'Search',
  other: 'Page',
}

// Simple domain-based favicon colors
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

interface TimelineEntryProps {
  entry: HistoryEntry
  isSelected: boolean
  onSelect: () => void
}

export function TimelineEntry({ entry, isSelected, onSelect }: TimelineEntryProps) {
  const time = entry.timestamp.toLocaleTimeString('en-US', { 
    hour: 'numeric', 
    minute: '2-digit',
    hour12: true 
  })

  return (
    <button
      onClick={onSelect}
      className={cn(
        'w-full flex gap-3 p-3 text-left rounded transition-all',
        isSelected 
          ? 'bg-oxblood-faint border border-oxblood/20' 
          : 'hover:bg-paper-hover border border-transparent'
      )}
    >
      {/* Time column */}
      <div className="w-16 flex-shrink-0 pt-0.5">
        <span className="text-xs font-mono text-ink-tertiary">{time}</span>
      </div>

      {/* Timeline dot */}
      <div className="flex flex-col items-center pt-1.5">
        <div className={cn(
          'w-2 h-2 rounded-full',
          entry.bookmarked ? 'bg-oxblood' : 'bg-ink-faint'
        )} />
      </div>

      {/* Content */}
      <div className="flex-1 min-w-0 space-y-2">
        {/* Header row */}
        <div className="flex items-center gap-2">
          {/* Favicon */}
          <div className={cn(
            'w-4 h-4 rounded flex items-center justify-center text-white text-2xs font-medium flex-shrink-0',
            getDomainColor(entry.domain)
          )}>
            {entry.domain.charAt(0).toUpperCase()}
          </div>
          <span className="text-xs text-ink-secondary truncate">{entry.domain}</span>
          <div className="flex-1" />
          <span className="type-badge">{typeLabels[entry.type]}</span>
          <button 
            className={cn(
              'p-1 rounded transition-colors',
              entry.bookmarked 
                ? 'text-oxblood' 
                : 'text-ink-faint hover:text-ink-secondary'
            )}
            onClick={(e) => {
              e.stopPropagation()
              // Toggle bookmark
            }}
          >
            <Bookmark className={cn('w-4 h-4', entry.bookmarked && 'fill-current')} />
          </button>
        </div>

        {/* Title */}
        <h3 className="font-serif text-base font-medium text-ink leading-snug text-balance">
          {entry.title}
        </h3>

        {/* Note */}
        {entry.note && (
          <div className="note-highlight">
            <FileEdit className="w-3.5 h-3.5 text-oxblood flex-shrink-0 mt-0.5" />
            <span className="text-oxblood font-medium text-xs">Note</span>
            <span className="text-ink-secondary text-sm">{entry.note}</span>
          </div>
        )}

        {/* Snippet */}
        {entry.snippet && (
          <div className="flex items-start gap-2">
            <Quote className="w-3.5 h-3.5 text-oxblood flex-shrink-0 mt-1 rotate-180" />
            <div>
              <span className="text-oxblood font-medium text-xs">Saved Snippet</span>
              <p className="snippet-quote mt-1">{entry.snippet}</p>
            </div>
          </div>
        )}
      </div>
    </button>
  )
}
