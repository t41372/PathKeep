import { useState } from 'react'
import { 
  Clock, 
  Search, 
  FolderOpen, 
  Layers, 
  Sparkles, 
  Settings,
  type LucideIcon
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { BrowsingIntensity } from './browsing-intensity'
import { LocalFirstCard } from './local-first-card'

interface NavItemProps {
  icon: LucideIcon
  label: string
  active?: boolean
  onClick?: () => void
}

function NavItem({ icon: Icon, label, active, onClick }: NavItemProps) {
  return (
    <button
      onClick={onClick}
      className={cn(
        'w-full flex items-center gap-3 px-3 py-2 text-sm rounded transition-colors',
        active 
          ? 'text-oxblood bg-oxblood-faint font-medium' 
          : 'text-ink-secondary hover:text-ink hover:bg-paper-hover'
      )}
    >
      <Icon className="w-4 h-4 flex-shrink-0" />
      <span>{label}</span>
    </button>
  )
}

export function Sidebar() {
  const [activeNav, setActiveNav] = useState('timeline')

  return (
    <aside className="w-56 flex-shrink-0 flex flex-col border-r border-border bg-paper">
      {/* Logo area */}
      <div className="px-4 py-5">
        <h1 className="font-serif text-xl font-semibold text-ink tracking-tight">
          PathKeep
        </h1>
        <p className="text-xs text-ink-tertiary font-mono mt-0.5">
          Editorial Atlas
        </p>
      </div>

      {/* Navigation */}
      <nav className="flex-1 px-2 space-y-0.5">
        <NavItem 
          icon={Clock} 
          label="Timeline" 
          active={activeNav === 'timeline'}
          onClick={() => setActiveNav('timeline')}
        />
        <NavItem 
          icon={Search} 
          label="Search" 
          active={activeNav === 'search'}
          onClick={() => setActiveNav('search')}
        />
        <NavItem 
          icon={FolderOpen} 
          label="Sources" 
          active={activeNav === 'sources'}
          onClick={() => setActiveNav('sources')}
        />
        <NavItem 
          icon={Layers} 
          label="Collections" 
          active={activeNav === 'collections'}
          onClick={() => setActiveNav('collections')}
        />
        <NavItem 
          icon={Sparkles} 
          label="Intelligence" 
          active={activeNav === 'intelligence'}
          onClick={() => setActiveNav('intelligence')}
        />
        <NavItem 
          icon={Settings} 
          label="Settings" 
          active={activeNav === 'settings'}
          onClick={() => setActiveNav('settings')}
        />
      </nav>

      {/* Browsing Intensity */}
      <div className="px-3 py-4 border-t border-border">
        <BrowsingIntensity />
      </div>

      {/* Local First Card */}
      <div className="px-3 pb-4">
        <LocalFirstCard />
      </div>
    </aside>
  )
}
