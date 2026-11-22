/** Status icon shared by the runs list and the run sheet. Mirrors `src/features/backup/run-icon.tsx`. */
import { CircleCheck, Loader2, TriangleAlert } from 'lucide-react'

export function RunIcon({ status }: { status: string }) {
  if (status === 'success') return <CircleCheck className="size-4 shrink-0 text-green" />
  if (status === 'running') return <Loader2 className="size-4 shrink-0 animate-spin text-muted-foreground" />
  return <TriangleAlert className={status === 'failed' ? 'size-4 shrink-0 text-red' : 'size-4 shrink-0 text-brand'} />
}
