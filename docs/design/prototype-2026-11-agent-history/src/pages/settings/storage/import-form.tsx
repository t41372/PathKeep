/**
 * Open a `.pathkeep` file here. Mirrors `ImportForm` and `BundleSummary` in
 * `src/features/settings/move-dialogs.tsx`: choose the file, check it, see
 * what is in it, type its password when it is encrypted, then replace (or,
 * on a computer with no archive yet, import).
 *
 * Prototype: a switch at the top picks which kind of file the check finds,
 * and whether this computer already has an archive; the password `wrong`
 * does not open the file.
 */
import { useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Spinner } from '@/components/ui/spinner'
import { NOW } from '~/data/fixture'
import { dateTime, size } from '../pathkeep-kit/format'
import { MaintenanceProgress, operationSteps, StoppedLine } from '../pathkeep-kit/maintenance-progress'
import { ErrorLine, Notice } from '../pathkeep-kit/notice'
import { appPaths, pathIn, usePlatform } from '../pathkeep-kit/platform'
import { ProtoBar, ProtoSelect, ProtoSwitch } from '../pathkeep-kit/proto-bar'
import { busyProps } from './move-dialogs'
import { PathField } from './path-field'

/** What `preview_app_data_import` reads from the file's manifest. */
interface Bundle {
  exportedAt: number
  hostname: string
  appVersion: string
  encrypted: boolean
  formatVersion: 1 | 2
  schemaUpToDate: boolean
  bytes: number
}

type Kind = 'current' | 'older' | 'v03' | 'v03plain'

const bundles: Record<Kind, Bundle> = {
  current: { exportedAt: NOW - 20 * 3600_000, hostname: 'Mei-MacBook-Air', appVersion: '0.6.0', encrypted: true, formatVersion: 2, schemaUpToDate: true, bytes: 3_318_000_000 },
  older: { exportedAt: NOW - 40 * 86_400_000, hostname: 'Mei-MacBook-Air', appVersion: '0.5.2', encrypted: true, formatVersion: 2, schemaUpToDate: false, bytes: 2_902_000_000 },
  v03: { exportedAt: NOW - 230 * 86_400_000, hostname: 'mei-desktop', appVersion: '0.3.4', encrypted: true, formatVersion: 1, schemaUpToDate: false, bytes: 1_640_000_000 },
  v03plain: { exportedAt: NOW - 260 * 86_400_000, hostname: 'mei-desktop', appVersion: '0.3.1', encrypted: false, formatVersion: 1, schemaUpToDate: false, bytes: 1_512_000_000 },
}

export function ImportForm({ onClose }: { onClose: () => void }) {
  const platform = usePlatform()
  const [kind, setKind] = useState<Kind>('current')
  const [hasArchive, setHasArchive] = useState(true)
  const [path, setPath] = useState('')
  const [password, setPassword] = useState('')
  const [checking, setChecking] = useState(false)
  const [preview, setPreview] = useState<Bundle | null>(null)
  const [applying, setApplying] = useState(false)
  const [stopped, setStopped] = useState(false)
  const [wrongPassword, setWrongPassword] = useState(false)
  const encrypted = preview?.encrypted ?? false
  const busy = checking || applying

  return (
    <DialogContent className="sm:max-w-md" {...busyProps(applying)}>
      <form
        className="flex flex-col gap-4"
        onSubmit={(event) => {
          event.preventDefault()
          if (busy) return
          if (!preview) {
            if (!path.trim()) return
            setChecking(true)
            setTimeout(() => {
              setChecking(false)
              setPreview(bundles[kind])
            }, 600)
          } else if (!encrypted || password) {
            setStopped(false)
            setWrongPassword(false)
            setApplying(true)
          }
        }}
      >
        <DialogHeader>
          <DialogTitle>開啟 PathKeep 檔案</DialogTitle>
          <DialogDescription>PathKeep 會先檢查檔案並顯示其中的內容。在你確認之前不會有任何變動。</DialogDescription>
        </DialogHeader>

        {!preview && (
          <ProtoBar>
            <ProtoSelect<Kind>
              label="檔案"
              value={kind}
              onChange={setKind}
              options={[
                { value: 'current', label: '0.6 匯出' },
                { value: 'older', label: '0.5 匯出（舊格式）' },
                { value: 'v03', label: '0.3 匯出，加密' },
                { value: 'v03plain', label: '0.3 匯出，沒加密' },
              ]}
            />
            <ProtoSwitch label="這台電腦已有存檔" checked={hasArchive} onChange={setHasArchive} />
            <span>原型：檔案密碼打 wrong 就算打錯</span>
          </ProtoBar>
        )}

        {!preview ? <PathField id="move-import-path" label="檔案" value={path} picked={pathIn(platform, appPaths(platform).downloads, 'pathkeep-2026-11-12.pathkeep')} onChange={setPath} disabled={busy} /> : <BundleSummary bundle={preview} />}

        {preview && encrypted && (
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="move-import-password">檔案密碼</Label>
            <Input
              id="move-import-password"
              type="password"
              autoComplete="off"
              value={password}
              disabled={applying}
              aria-invalid={wrongPassword}
              aria-describedby={preview.formatVersion === 1 ? 'move-import-password-hint' : undefined}
              onChange={(event) => setPassword(event.target.value)}
            />
            {preview.formatVersion === 1 && (
              <p id="move-import-password-hint" className="text-xs text-muted-foreground">
                這個檔案由 PathKeep 0.3 匯出：它的密碼就是你在那台電腦上用的存檔密碼。
              </p>
            )}
          </div>
        )}

        {preview && hasArchive && <Notice tone="warning">這會取代這台電腦上的存檔。目前的存檔會以備份資料夾的形式保留在旁邊。</Notice>}

        {applying && (
          <MaintenanceProgress
            steps={operationSteps.import}
            onDone={() => {
              setApplying(false)
              if (encrypted && password === 'wrong') {
                setWrongPassword(true)
                return
              }
              toast.success('匯入完成。')
              toast('原型：PathKeep 在這裡重新開始，換上剛開啟的存檔。')
              onClose()
            }}
            onStopped={() => {
              setApplying(false)
              setStopped(true)
            }}
          />
        )}
        {stopped && <StoppedLine />}
        {wrongPassword && <ErrorLine>這組密碼打不開這個檔案。</ErrorLine>}

        <DialogFooter>
          <Button
            type="button"
            variant="ghost"
            disabled={busy}
            onClick={
              preview
                ? () => {
                    setPreview(null)
                    setWrongPassword(false)
                    setStopped(false)
                  }
                : onClose
            }
          >
            {preview ? '上一步' : '取消'}
          </Button>
          {preview ? (
            <Button type="submit" variant={hasArchive ? 'destructive' : 'default'} disabled={busy || (encrypted && !password)}>
              {hasArchive ? '取代我的存檔' : '匯入'}
            </Button>
          ) : (
            <Button type="submit" disabled={busy || !path.trim()}>
              {checking && <Spinner />}
              檢查檔案
            </Button>
          )}
        </DialogFooter>
      </form>
    </DialogContent>
  )
}

function BundleSummary({ bundle }: { bundle: Bundle }) {
  const rows: [string, string][] = [
    ['製作時間', [dateTime(bundle.exportedAt), bundle.hostname].filter(Boolean).join(' · ')],
    ['PathKeep 版本', bundle.appVersion],
    ['已加密', bundle.encrypted ? '是' : '否'],
    ['大小', size(bundle.bytes)],
  ]
  return (
    <div className="flex flex-col gap-2 rounded-lg bg-muted p-3 text-[13px]">
      {rows.map(([label, value]) => (
        <div key={label} className="flex justify-between gap-4">
          <span className="text-muted-foreground">{label}</span>
          <span className="text-right">{value}</span>
        </div>
      ))}
      {!bundle.schemaUpToDate && <p className="text-muted-foreground">它由較舊版本的 PathKeep 製作，開啟時會自動更新。</p>}
      {bundle.formatVersion === 1 && <p className="text-muted-foreground">PathKeep 0.3 匯出的檔案不含助理對話；它們仍留在原來的電腦上。</p>}
    </div>
  )
}
