/**
 * Copy and "show in folder" for every path Backup names (audit records,
 * manifests, kept copies). Mirrors `src/features/backup/path-actions.tsx`:
 * the copy button turns into a check for 1.5 s; "show in folder" appears only
 * for a real filesystem path, not a label like `Task Scheduler:<task>`.
 */
import { Check, Copy, FolderOpen } from 'lucide-react'
import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/cn'

const isFilesystemPath = (path: string) => path.startsWith('/') || /^[A-Za-z]:[\\/]/.test(path)

export function CopyButton({ text, label, className }: { text: string; label: string; className?: string }) {
  const [copied, setCopied] = useState(false)
  useEffect(() => {
    if (!copied) return
    const timer = window.setTimeout(() => setCopied(false), 1500)
    return () => window.clearTimeout(timer)
  }, [copied])

  return (
    <Button
      type="button"
      size="icon-xs"
      variant="ghost"
      aria-label={copied ? '已複製' : label}
      title={label}
      className={cn('text-muted-foreground', className)}
      onClick={() => {
        // The app toasts a refused clipboard; the prototype shows the check either way.
        void navigator.clipboard?.writeText(text).catch(() => undefined)
        setCopied(true)
      }}
    >
      {copied ? <Check /> : <Copy />}
    </Button>
  )
}

export function RevealButton() {
  return (
    <Button
      type="button"
      size="icon-xs"
      variant="ghost"
      aria-label="在資料夾中顯示"
      title="在資料夾中顯示"
      className="text-muted-foreground"
      onClick={() => toast('原型：這裡會在 Finder 中打開它所在的資料夾')}
    >
      <FolderOpen />
    </Button>
  )
}

export function PathLine({ path, className }: { path: string; className?: string }) {
  return (
    <div className={cn('flex items-start gap-1', className)}>
      <span className="min-w-0 flex-1 pt-0.5 font-mono text-xs leading-5 break-all">{path}</span>
      <CopyButton text={path} label="複製路徑" />
      {isFilesystemPath(path) && <RevealButton />}
    </div>
  )
}
