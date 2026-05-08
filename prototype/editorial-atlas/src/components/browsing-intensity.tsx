import { useState } from 'react'
import { Info } from 'lucide-react'
import { cn } from '@/lib/utils'
import { browsingIntensityData } from '@/lib/mock-data'

type Period = 'day' | 'week' | 'month' | 'year'

const periodLabels: Record<Period, string> = {
  day: 'Day',
  week: 'Week', 
  month: 'Month',
  year: 'Year',
}

const timeLabels = ['12 AM', '6 AM', '12 PM', '6 PM']
const dayLabels = ['M', 'T', 'W', 'T', 'F', 'S', 'S']

function IntensityCell({ level }: { level: number }) {
  const intensityColors = [
    'bg-transparent',
    'bg-oxblood/10',
    'bg-oxblood/25',
    'bg-oxblood/45',
    'bg-oxblood/70',
  ]

  return (
    <div 
      className={cn(
        'w-4 h-4 rounded-sm transition-colors',
        intensityColors[level] || intensityColors[0]
      )}
    />
  )
}

export function BrowsingIntensity() {
  const [period, setPeriod] = useState<Period>('week')

  return (
    <div className="space-y-3">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-1.5">
          <span className="text-xs font-medium text-ink">Browsing Intensity</span>
          <Info className="w-3 h-3 text-ink-tertiary" />
        </div>
      </div>

      {/* Period tabs */}
      <div className="flex gap-0.5 p-0.5 bg-paper-card rounded border border-border">
        {(Object.keys(periodLabels) as Period[]).map((p) => (
          <button
            key={p}
            onClick={() => setPeriod(p)}
            className={cn(
              'flex-1 px-2 py-1 text-2xs font-medium rounded transition-colors',
              period === p 
                ? 'bg-paper text-ink shadow-sm' 
                : 'text-ink-tertiary hover:text-ink-secondary'
            )}
          >
            {periodLabels[p]}
          </button>
        ))}
      </div>

      {/* Heatmap grid */}
      <div className="space-y-1">
        {/* Day labels */}
        <div className="flex gap-1 pl-8">
          {dayLabels.map((day, i) => (
            <div key={i} className="w-4 text-center text-2xs text-ink-tertiary">
              {day}
            </div>
          ))}
        </div>

        {/* Grid rows */}
        {browsingIntensityData.map((row, rowIndex) => (
          <div key={rowIndex} className="flex items-center gap-1">
            <span className="w-7 text-2xs text-ink-tertiary text-right pr-1">
              {timeLabels[rowIndex]}
            </span>
            {row.map((level, colIndex) => (
              <IntensityCell key={colIndex} level={level} />
            ))}
          </div>
        ))}
      </div>

      {/* Legend */}
      <p className="text-2xs text-ink-tertiary italic">
        Darker tones represent more browsing activity.
      </p>
    </div>
  )
}
