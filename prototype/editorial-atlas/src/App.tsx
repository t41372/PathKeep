import { useState } from 'react'
import { Sidebar } from './components/sidebar'
import { TopBar } from './components/top-bar'
import { Timeline } from './components/timeline'
import { DetailPanel } from './components/detail-panel'
import { StatusBar } from './components/status-bar'
import { mockEntries, groupEntriesByDay, type HistoryEntry } from './lib/mock-data'

function App() {
  const [selectedEntry, setSelectedEntry] = useState<HistoryEntry | null>(mockEntries[0])
  const groups = groupEntriesByDay(mockEntries)

  return (
    <div className="h-screen flex flex-col bg-paper">
      {/* Window chrome simulation for macOS feel */}
      <div className="h-8 flex items-center px-4 border-b border-border bg-paper-card flex-shrink-0">
        <div className="flex items-center gap-2">
          <div className="w-3 h-3 rounded-full bg-[#FF5F57]" />
          <div className="w-3 h-3 rounded-full bg-[#FEBC2E]" />
          <div className="w-3 h-3 rounded-full bg-[#28C840]" />
        </div>
      </div>

      {/* Main layout */}
      <div className="flex-1 flex overflow-hidden">
        {/* Sidebar */}
        <Sidebar />

        {/* Main content */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Top bar */}
          <TopBar />

          {/* Content area */}
          <div className="flex-1 flex overflow-hidden">
            {/* Timeline */}
            <Timeline 
              groups={groups}
              selectedEntry={selectedEntry}
              onSelectEntry={setSelectedEntry}
            />

            {/* Detail panel */}
            <DetailPanel entry={selectedEntry} />
          </div>

          {/* Status bar */}
          <StatusBar />
        </div>
      </div>
    </div>
  )
}

export default App
