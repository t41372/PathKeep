import { TimelineEntry } from './timeline-entry'
import type { DayGroup, HistoryEntry } from '@/lib/mock-data'

interface TimelineProps {
  groups: DayGroup[]
  selectedEntry: HistoryEntry | null
  onSelectEntry: (entry: HistoryEntry) => void
}

export function Timeline({ groups, selectedEntry, onSelectEntry }: TimelineProps) {
  return (
    <div className="flex-1 overflow-y-auto">
      <div className="max-w-2xl mx-auto py-4 px-4 space-y-6">
        {groups.map((group) => (
          <section key={group.date.toISOString()}>
            {/* Day header */}
            <div className="flex items-center gap-3 mb-3">
              <span className="text-sm font-semibold text-oxblood">
                {group.label.split(',')[0]}
              </span>
              <span className="text-sm text-ink-tertiary">
                {group.label.includes(',') 
                  ? group.label.split(',').slice(1).join(',').trim()
                  : group.date.toLocaleDateString('en-US', { 
                      month: 'short', 
                      day: 'numeric',
                      year: 'numeric'
                    })
                }
              </span>
              <div className="flex-1 h-px bg-border" />
            </div>

            {/* Entries */}
            <div className="space-y-1">
              {group.entries.map((entry) => (
                <TimelineEntry
                  key={entry.id}
                  entry={entry}
                  isSelected={selectedEntry?.id === entry.id}
                  onSelect={() => onSelectEntry(entry)}
                />
              ))}
            </div>
          </section>
        ))}

        {/* Empty state */}
        {groups.length === 0 && (
          <div className="py-20 text-center">
            <p className="font-serif text-lg text-ink-secondary italic">
              Nothing here yet.
            </p>
            <p className="text-sm text-ink-tertiary mt-2">
              Memory is patient.
            </p>
          </div>
        )}
      </div>
    </div>
  )
}
