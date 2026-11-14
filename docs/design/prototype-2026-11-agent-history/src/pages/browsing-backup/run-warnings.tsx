/**
 * Draws a run's warnings (their own tab) and notes (under the summary), and
 * what an import recorded (in its preview and its review), each with the fix
 * it points to. Mirrors `src/features/backup/run-warnings.tsx`.
 */
import { Info, TriangleAlert } from 'lucide-react'
import type { ReactNode } from 'react'
import { Link } from 'react-router-dom'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { describe, type Recorded, type RunAction, type RunEntry } from './run-entries'

function ActionButton({ action }: { action: RunAction }) {
  if (action === 'full-disk-access')
    return (
      <Button
        size="sm"
        variant="outline"
        className="self-start"
        onClick={() => toast('原型：這裡會打開「系統設定 → 隱私權與安全性 → 完整磁碟取用權限」')}
      >
        授權
      </Button>
    )
  return (
    <Button asChild size="sm" variant="outline" className="self-start">
      <Link to={action === 'ai-settings' ? '/settings/ai' : '/settings/background'}>
        {action === 'ai-settings' ? '前往 AI 設定' : '前往背景工作'}
      </Link>
    </Button>
  )
}

function Entry({ entry, icon }: { entry: RunEntry; icon: ReactNode }) {
  return (
    <li className="flex gap-2 rounded-lg bg-muted px-3 py-2">
      {icon}
      <div className="flex min-w-0 flex-1 flex-col gap-1.5">
        <p className="break-words">{entry.text}</p>
        {entry.evidence && (
          <p className="font-mono text-[11px] break-words whitespace-pre-wrap text-muted-foreground">{entry.evidence}</p>
        )}
        {entry.action && <ActionButton action={entry.action} />}
      </div>
    </li>
  )
}

const WARNING_ICON = <TriangleAlert className="mt-0.5 size-4 shrink-0 text-brand" />
const NOTE_ICON = <Info className="mt-0.5 size-4 shrink-0 text-muted-foreground" />

function Entries({ entries, icon }: { entries: RunEntry[]; icon: ReactNode }) {
  return (
    <ul className="flex flex-col gap-1.5 text-[13px]">
      {entries.map((entry, index) => (
        <Entry key={`${index}-${entry.text}`} entry={entry} icon={icon} />
      ))}
    </ul>
  )
}

export function WarningList({ entries }: { entries: RunEntry[] }) {
  return entries.length === 0 ? <p className="text-[13px] text-muted-foreground">沒有警告。</p> : <Entries entries={entries} icon={WARNING_ICON} />
}

export function NoteList({ entries }: { entries: RunEntry[] }) {
  if (entries.length === 0) return null
  return (
    <section className="flex flex-col gap-1.5">
      <h3 className="text-[13px] font-medium">這次執行還做了</h3>
      <Entries entries={entries} icon={NOTE_ICON} />
    </section>
  )
}

/** What an import recorded: warnings, then notes; nothing when it recorded nothing. */
export function ImportEntries({ recorded, skip }: { recorded: Recorded[]; skip?: readonly string[] }) {
  const { warnings, notes } = describe(recorded, skip)
  if (warnings.length === 0 && notes.length === 0) return null
  return (
    <div className="flex flex-col gap-1.5">
      {warnings.length > 0 && <Entries entries={warnings} icon={WARNING_ICON} />}
      {notes.length > 0 && <Entries entries={notes} icon={NOTE_ICON} />}
    </div>
  )
}
