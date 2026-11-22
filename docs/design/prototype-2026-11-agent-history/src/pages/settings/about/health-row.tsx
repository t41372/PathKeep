/**
 * 檢查存檔: the read-only checks and, when one fails, the repair behind a
 * confirmation. Mirrors `src/features/settings/health-row.tsx`. The checks
 * and their order are the backend's (`vault-core/src/archive/doctor.rs`);
 * whether they pass comes from the section's prototype switch, and a repair
 * makes them pass.
 */
import { CircleAlert, CircleCheck } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'
import { SettingRow } from '@/components/app/setting-row'
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { Spinner } from '@/components/ui/spinner'

export type HealthOutcome = 'none' | 'pass' | 'fail'

const checks: { code: string; name: string; failDetail?: string }[] = [
  { code: 'config', name: '設定檔' },
  { code: 'browser-sources', name: '瀏覽器' },
  { code: 'archive-db', name: '存檔檔案' },
  { code: 'archive-unlock', name: '存檔能否解鎖' },
  { code: 'archive-integrity', name: '存檔檔案完好' },
  { code: 'source-evidence-integrity', name: '瀏覽器原始記錄完好' },
  { code: 'intelligence-integrity', name: '洞察資料完好' },
  { code: 'agent-integrity', name: '已儲存的對話完好' },
  { code: 'schema-version', name: '存檔格式' },
  { code: 'canonical-references', name: '頁面與瀏覽記錄相互對應' },
  { code: 'broken-visibility-references', name: '已隱藏的記錄' },
  { code: 'cached-totals', name: '統計數字與存檔一致', failDetail: '3 counters differ from facts' },
  { code: 'search-outbox', name: '搜尋跟得上存檔', failDetail: '12 missing documents or invalid latest events' },
  { code: 'manifest-chain', name: '備份記錄' },
  { code: 'snapshot-artifacts', name: '快照' },
  { code: 'import-audit-artifacts', name: '匯入記錄' },
  { code: 'derived-state-freshness', name: '搜尋和洞察資料' },
]

export function HealthRow({ outcome, onOutcome, failNext }: { outcome: HealthOutcome; onOutcome: (outcome: HealthOutcome) => void; failNext: boolean }) {
  const [confirm, setConfirm] = useState(false)
  const [checking, setChecking] = useState(false)
  const [repairing, setRepairing] = useState(false)
  const busy = checking || repairing
  const report = outcome === 'none' ? null : checks.map((item) => ({ ...item, ok: outcome === 'pass' || !item.failDetail }))
  const failing = report?.filter((item) => !item.ok) ?? []

  const run = (result: HealthOutcome) => {
    setChecking(true)
    setTimeout(() => {
      setChecking(false)
      onOutcome(result)
    }, 800)
  }

  const repair = () => {
    setRepairing(true)
    setTimeout(() => {
      setRepairing(false)
      toast.success('修復完成。')
      run('pass')
    }, 1000)
  }

  return (
    <SettingRow
      title="檢查存檔"
      description={report ? (failing.length === 0 ? `全部 ${report.length} 項檢查通過。` : `發現 ${failing.length} 個問題。`) : '尋找遺失的檔案和損壞的記錄，不會做任何變動。'}
      control={
        <>
          {failing.length > 0 && (
            <Button size="sm" variant="outline" disabled={busy} onClick={() => setConfirm(true)}>
              {repairing && <Spinner />}
              修復
            </Button>
          )}
          <Button size="sm" variant="outline" disabled={busy} onClick={() => run(failNext ? 'fail' : 'pass')}>
            {checking && <Spinner />}
            {report ? '再檢查一次' : '檢查'}
          </Button>
        </>
      }
    >
      {report && (
        <ul className="flex flex-col gap-1.5 border-t pt-3 text-[13px]">
          {report.map((item) => (
            <li key={item.code} className="flex gap-2">
              {item.ok ? <CircleCheck className="mt-0.5 size-4 shrink-0 text-green" aria-label="通過" /> : <CircleAlert className="mt-0.5 size-4 shrink-0 text-destructive" aria-label="有問題" />}
              <span className="flex min-w-0 flex-col">
                <span>{item.name}</span>
                {!item.ok && item.failDetail && <span className="text-xs break-words text-muted-foreground">{item.failDetail}</span>}
              </span>
            </li>
          ))}
        </ul>
      )}
      <AlertDialog open={confirm} onOpenChange={setConfirm}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>要修復存檔嗎？</AlertDialogTitle>
            <AlertDialogDescription>PathKeep 會重建能重建的記錄，並清除過時的搜尋和洞察資料以便重新產生。你的歷史不會被刪除。</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction onClick={repair}>修復</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </SettingRow>
  )
}
