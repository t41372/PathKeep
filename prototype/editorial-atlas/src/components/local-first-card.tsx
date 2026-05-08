import { HardDrive } from 'lucide-react'

export function LocalFirstCard() {
  return (
    <div className="p-3 bg-paper-card border border-border rounded">
      <div className="flex items-start gap-2">
        <div className="w-2 h-2 mt-1.5 rounded-full bg-oxblood animate-pulse" />
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-1.5">
            <HardDrive className="w-3.5 h-3.5 text-oxblood" />
            <span className="text-sm font-medium text-oxblood">Local First</span>
          </div>
          <p className="text-xs text-ink-secondary mt-1 leading-relaxed">
            All data is stored on this device and never leaves your control.
          </p>
          <button className="text-xs text-oxblood hover:underline mt-1.5">
            Learn more
          </button>
        </div>
      </div>
    </div>
  )
}
