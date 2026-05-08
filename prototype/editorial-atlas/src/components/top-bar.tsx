import { useState } from 'react'
import { 
  Search, 
  Calendar, 
  ChevronDown, 
  SlidersHorizontal,
  Upload,
  MessageCircle,
  X
} from 'lucide-react'
import { cn } from '@/lib/utils'

interface FilterDropdownProps {
  label: string
  value: string
  options: string[]
}

function FilterDropdown({ label, value, options }: FilterDropdownProps) {
  const [isOpen, setIsOpen] = useState(false)
  const [selected, setSelected] = useState(value)

  return (
    <div className="relative">
      <button 
        onClick={() => setIsOpen(!isOpen)}
        className="flex items-center gap-1.5 px-2.5 py-1.5 text-sm text-ink-secondary hover:text-ink border border-border rounded hover:border-border-strong transition-colors"
      >
        <span>{selected}</span>
        <ChevronDown className="w-3.5 h-3.5" />
      </button>
      
      {isOpen && (
        <>
          <div 
            className="fixed inset-0 z-10" 
            onClick={() => setIsOpen(false)} 
          />
          <div className="absolute top-full left-0 mt-1 py-1 bg-paper border border-border rounded shadow-card-hover z-20 min-w-[140px]">
            {options.map((option) => (
              <button
                key={option}
                onClick={() => {
                  setSelected(option)
                  setIsOpen(false)
                }}
                className={cn(
                  'w-full px-3 py-1.5 text-sm text-left hover:bg-paper-hover transition-colors',
                  selected === option ? 'text-oxblood font-medium' : 'text-ink-secondary'
                )}
              >
                {option}
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  )
}

export function TopBar() {
  return (
    <header className="flex items-center justify-between gap-4 px-4 py-3 border-b border-border bg-paper">
      {/* Search */}
      <div className="flex-1 max-w-md">
        <div className="relative">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-ink-tertiary" />
          <input
            type="text"
            placeholder="Search your history..."
            className="input pl-9 pr-12"
          />
          <kbd className="absolute right-3 top-1/2 -translate-y-1/2 px-1.5 py-0.5 text-2xs font-mono text-ink-tertiary bg-paper-card border border-border rounded">
            ⌘K
          </kbd>
        </div>
      </div>

      {/* Date range */}
      <button className="flex items-center gap-2 px-3 py-1.5 text-sm text-ink-secondary hover:text-ink border border-border rounded hover:border-border-strong transition-colors">
        <Calendar className="w-4 h-4" />
        <span>May 18 – May 24, 2026</span>
      </button>

      {/* Filters */}
      <div className="flex items-center gap-2">
        <FilterDropdown 
          label="Source"
          value="All Sources"
          options={['All Sources', 'Chrome', 'Firefox', 'Safari', 'Arc Browser', 'Edge']}
        />
        <FilterDropdown 
          label="Type"
          value="All Types"
          options={['All Types', 'Article', 'Repository', 'Video', 'Docs', 'Social', 'Search']}
        />
        <button className="p-1.5 text-ink-tertiary hover:text-ink hover:bg-paper-hover rounded transition-colors">
          <SlidersHorizontal className="w-4 h-4" />
        </button>
      </div>

      {/* Actions */}
      <div className="flex items-center gap-1">
        <button className="p-2 text-ink-tertiary hover:text-ink hover:bg-paper-hover rounded transition-colors">
          <Upload className="w-4 h-4" />
        </button>
        <button className="p-2 text-ink-tertiary hover:text-ink hover:bg-paper-hover rounded transition-colors">
          <MessageCircle className="w-4 h-4" />
        </button>
        <button className="p-2 text-ink-tertiary hover:text-ink hover:bg-paper-hover rounded transition-colors">
          <X className="w-4 h-4" />
        </button>
      </div>
    </header>
  )
}
