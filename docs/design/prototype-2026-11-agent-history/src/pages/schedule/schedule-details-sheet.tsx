/**
 * 自動備份詳情: what PathKeep verified about the automatic backup. The checks,
 * problems, files found, the last install / remove and its record, recent
 * runs the scheduler started, and the manual steps. Mirrors
 * `src/features/backup/schedule-details-sheet.tsx`.
 */
import { CircleAlert, CircleCheck, CircleDashed, RefreshCw, TriangleAlert } from 'lucide-react'
import type { ReactNode } from 'react'
import type { ScheduleIssue, ScheduleStatus, ScheduleVerificationCheck } from '@/lib/types'
import { Button } from '@/components/ui/button'
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@/components/ui/sheet'
import { cn } from '@/lib/cn'
import { dayAndTime } from './format'
import { ManualSteps, PathLine } from './plan-view'
import { scheduleCopy } from './schedule-copy'

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex min-w-0 flex-col gap-2">
      <h3 className="text-[13px] font-medium">{title}</h3>
      {children}
    </section>
  )
}

function Empty({ children }: { children: ReactNode }) {
  return <p className="text-[13px] text-muted-foreground">{children}</p>
}

const checkIcons = {
  ok: <CircleCheck className="size-4 text-green" />,
  warning: <TriangleAlert className="size-4 text-brand" />,
  error: <CircleAlert className="size-4 text-destructive" />,
  pending: <CircleDashed className="size-4 text-muted-foreground" />,
}
const checkStatus = { ok: '正常', warning: '注意', error: '有問題', pending: '尚未' }

function CheckRow({ check }: { check: ScheduleVerificationCheck }) {
  const status = check.status in checkIcons ? check.status : 'pending'
  return (
    <li className="flex items-start gap-2.5 py-1.5">
      <span className="mt-0.5" aria-hidden>
        {checkIcons[status]}
      </span>
      <div className="flex min-w-0 flex-1 flex-col gap-0.5 text-[13px]">
        <span className="flex items-baseline justify-between gap-3">
          <span className="font-medium">{scheduleCopy(check.labelKey) ?? check.key}</span>
          <span className="shrink-0 text-xs text-muted-foreground">{checkStatus[status]}</span>
        </span>
        {scheduleCopy(check.detailKey) && <span className="text-muted-foreground">{scheduleCopy(check.detailKey)}</span>}
        {check.evidence.length > 0 && <span className="font-mono text-[11px] break-all text-muted-foreground">{check.evidence.join(' · ')}</span>}
      </div>
    </li>
  )
}

function IssueRow({ issue }: { issue: ScheduleIssue }) {
  return (
    <li className={cn('flex flex-col gap-0.5 rounded-lg p-3 text-[13px]', issue.severity === 'error' ? 'bg-destructive/10' : 'bg-brand-soft')}>
      <span className="font-medium">{scheduleCopy(issue.titleKey) ?? issue.code}</span>
      {[issue.detailKey, issue.consequenceKey].map((key) => {
        const text = scheduleCopy(key)
        return text ? (
          <span key={key} className="text-muted-foreground">
            {text}
          </span>
        ) : null
      })}
      {issue.evidence.length > 0 && <span className="font-mono text-[11px] break-all text-muted-foreground">{issue.evidence.join(' · ')}</span>}
    </li>
  )
}

const attemptLabels: Record<string, string> = {
  running: '執行中',
  success: '已備份',
  partial: '備份未完成',
  skipped: '還沒到時間',
  deferred: '等待另一個備份',
  failed: '失敗',
  interrupted: '中途停止',
}
const actionLabels: Record<string, string> = { apply: '已安裝', remove: '已移除', repair: '已修復' }
const outcomeLabels: Record<string, string> = { ok: '完成', failed: '失敗', unknown: '沒有記錄結果' }

function LastChange({ status }: { status: ScheduleStatus }) {
  const last = status.lastAction
  if (!last) return <Empty>PathKeep 還沒有安裝或移除過工作。</Empty>
  return (
    <div className="flex flex-col gap-1.5 text-[13px]">
      <span>
        <span className="font-medium">{actionLabels[last.action] ?? last.action}</span>
        <span className="text-muted-foreground">
          {' · '}
          {outcomeLabels[last.status] ?? outcomeLabels.unknown}
          {' · '}
          {dayAndTime(Date.parse(last.at))}
        </span>
      </span>
      {(last.files ?? []).map((file) => (
        <PathLine key={file} path={file} />
      ))}
      {last.auditPath && (
        <div className="flex flex-col gap-0.5">
          <span className="text-xs text-muted-foreground">記錄</span>
          <PathLine path={last.auditPath} />
        </div>
      )}
    </div>
  )
}

export function ScheduleDetailsSheet({ open, status, checking, onCheck, onClose }: { open: boolean; status: ScheduleStatus; checking: boolean; onCheck: () => void; onClose: () => void }) {
  const checks = status.verificationChecks ?? []
  const issues = status.issues ?? []
  const attempts = status.recentAttempts ?? []
  return (
    <Sheet open={open} onOpenChange={(next) => !next && onClose()}>
      <SheetContent className="w-[460px] overflow-y-auto sm:max-w-[460px]">
        <SheetHeader>
          <SheetTitle>自動備份詳情</SheetTitle>
          <SheetDescription>PathKeep 上次檢查系統排程器時看到的情況。</SheetDescription>
        </SheetHeader>
        <div className="flex flex-col gap-5 px-4 pb-6">
          <div className="flex items-center justify-between gap-3 text-[13px] text-muted-foreground">
            <span>{status.checkedAt ? `檢查於 ${dayAndTime(Date.parse(status.checkedAt))}` : null}</span>
            <Button size="sm" variant="outline" disabled={checking} onClick={onCheck}>
              <RefreshCw className={cn(checking && 'animate-spin')} />
              {checking ? '正在檢查…' : '再檢查一次'}
            </Button>
          </div>

          {issues.length > 0 && (
            <Section title="問題">
              <ul className="flex flex-col gap-2">
                {issues.map((issue) => (
                  <IssueRow key={`${issue.code}-${issue.titleKey}`} issue={issue} />
                ))}
              </ul>
            </Section>
          )}

          <Section title="檢查項目">
            {checks.length === 0 ? (
              <Empty>沒有可檢查的：自動備份還沒有安裝。</Empty>
            ) : (
              <ul className="flex flex-col">
                {checks.map((check) => (
                  <CheckRow key={check.key} check={check} />
                ))}
              </ul>
            )}
          </Section>

          <Section title="找到的檔案">
            {status.detectedFiles.length === 0 ? (
              <Empty>這台電腦上沒有 PathKeep 的排程檔案。</Empty>
            ) : (
              <div className="flex flex-col gap-1">
                {status.detectedFiles.map((file) => (
                  <PathLine key={file} path={file} />
                ))}
              </div>
            )}
          </Section>

          <Section title="最近一次變更">
            <LastChange status={status} />
          </Section>

          <Section title="最近的自動執行">
            {attempts.length === 0 ? (
              <Empty>排程器還沒有啟動過 PathKeep。</Empty>
            ) : (
              <ul className="flex flex-col text-[13px]">
                {attempts.slice(0, 8).map((attempt) => (
                  <li key={attempt.id} className="flex justify-between gap-3 py-1">
                    <span className={cn(attempt.outcome === 'partial' && 'text-brand', (attempt.outcome === 'failed' || attempt.outcome === 'interrupted') && 'text-destructive')}>
                      {attemptLabels[attempt.outcome] ?? attempt.outcome}
                    </span>
                    <span className="font-mono text-xs text-muted-foreground tabular">{dayAndTime(Date.parse(attempt.startedAt))}</span>
                  </li>
                ))}
              </ul>
            )}
          </Section>

          <Section title="自己設定">
            <p className="text-[13px] text-muted-foreground">想手動設定？依這些步驟可以裝上和 PathKeep 一樣的工作。</p>
            <ManualSteps details={status.manualStepDetails} steps={status.manualSteps} />
          </Section>
        </div>
      </SheetContent>
    </Sheet>
  )
}
