/**
 * The files of a schedule plan as the scheduler will get them, and the manual
 * steps that install the same thing by hand; each path with copy and show in
 * folder. Mirrors `src/features/backup/schedule-plan-view.tsx` and
 * `path-actions.tsx`.
 */
import { Check, Copy, FolderOpen } from 'lucide-react'
import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import type { ScheduleManualStep, SchedulePlan } from '@/lib/types'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/cn'
import { scheduleCopy } from './schedule-copy'

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
      onClick={() => void navigator.clipboard.writeText(text).then(() => setCopied(true), () => setCopied(true))}
    >
      {copied ? <Check /> : <Copy />}
    </Button>
  )
}

function RevealButton({ path }: { path: string }) {
  return (
    <Button
      type="button"
      size="icon-xs"
      variant="ghost"
      aria-label="在資料夾中顯示"
      title="在資料夾中顯示"
      className="text-muted-foreground"
      onClick={() => toast(`原型：在資料夾中顯示 ${path}`)}
    >
      <FolderOpen />
    </Button>
  )
}

/** One path in monospace with copy and, for real paths, show-in-folder. */
export function PathLine({ path, className }: { path: string; className?: string }) {
  return (
    <div className={cn('flex items-start gap-1', className)}>
      <span className="min-w-0 flex-1 pt-0.5 font-mono text-xs leading-5 break-all">{path}</span>
      <CopyButton text={path} label="複製路徑" />
      {isFilesystemPath(path) && <RevealButton path={path} />}
    </div>
  )
}

function FileBlock({ path, contents }: { path: string; contents: string }) {
  return (
    <div className="flex min-w-0 flex-col gap-1.5">
      <PathLine path={path} />
      <div className="relative">
        <pre aria-label="檔案內容" className="max-h-56 overflow-auto rounded-lg bg-muted p-3 pr-10 font-mono text-[11px] leading-relaxed whitespace-pre">
          {contents}
        </pre>
        <CopyButton text={contents} label="複製檔案" className="absolute top-1.5 right-1.5 bg-muted" />
      </div>
    </div>
  )
}

/** Where a generated file goes, from the manual step that saves it. */
function destination(plan: SchedulePlan, contents: string, fallback: string) {
  return plan.manualStepDetails?.find((s) => s.fileContents === contents)?.filePath ?? fallback
}

export function PlanFiles({ plan }: { plan: SchedulePlan }) {
  return (
    <div className="flex flex-col gap-3">
      {plan.generatedFiles.map((file) => (
        <FileBlock key={file.relativePath} path={destination(plan, file.contents, file.absolutePath ?? file.relativePath)} contents={file.contents} />
      ))}
    </div>
  )
}

function StepBody({ step }: { step: ScheduleManualStep }) {
  const command = step.command?.join(' ')
  return (
    <div className="flex min-w-0 flex-col gap-1.5">
      <span className="font-medium">{scheduleCopy(step.titleKey) ?? step.id}</span>
      {scheduleCopy(step.summaryKey) && (
        <span className="text-muted-foreground">
          {scheduleCopy(step.summaryKey)} {scheduleCopy(step.whyKey)}
        </span>
      )}
      {step.fileContents && step.filePath && <FileBlock path={step.filePath} contents={step.fileContents} />}
      {command && (
        <div className="flex items-start gap-1 rounded-lg bg-muted px-3 py-2">
          <code className="min-w-0 flex-1 font-mono text-[11px] leading-5 break-all">{command}</code>
          <CopyButton text={command} label="複製指令" />
        </div>
      )}
    </div>
  )
}

/** Numbered manual steps; falls back to the plain strings when there are no details. */
export function ManualSteps({ details, steps }: { details?: ScheduleManualStep[]; steps: string[] }) {
  if (details && details.length > 0) {
    return (
      <ol className="flex list-decimal flex-col gap-3 pl-4 text-[13px]">
        {details.map((step) => (
          <li key={step.id}>
            <StepBody step={step} />
          </li>
        ))}
      </ol>
    )
  }
  return (
    <ol className="flex list-decimal flex-col gap-1 pl-4 font-mono text-xs break-all">
      {steps.map((step) => (
        <li key={step}>{step}</li>
      ))}
    </ol>
  )
}
