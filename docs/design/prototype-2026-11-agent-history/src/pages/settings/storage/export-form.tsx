/**
 * Pack PathKeep into one file. Mirrors `ExportForm` in
 * `src/features/settings/move-dialogs.tsx`: where to save (Downloads by
 * default), the file password twice, the run with progress and Stop, and a
 * toast that can show the file.
 */
import { useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { size } from '../pathkeep-kit/format'
import { MaintenanceProgress, operationSteps, StoppedLine } from '../pathkeep-kit/maintenance-progress'
import { appPaths, pathIn, usePlatform } from '../pathkeep-kit/platform'
import { busyProps } from './move-dialogs'
import { BUNDLE_EXTENSION, PathField } from './path-field'

export function ExportForm({ onClose }: { onClose: () => void }) {
  const platform = usePlatform()
  const paths = appPaths(platform)
  const name = `pathkeep-2026-11-13.${BUNDLE_EXTENSION}`
  const [path, setPath] = useState(pathIn(platform, paths.downloads, name))
  const [password, setPassword] = useState('')
  const [confirm, setConfirm] = useState('')
  const [running, setRunning] = useState(false)
  const [stopped, setStopped] = useState(false)
  const differ = confirm !== '' && confirm !== password
  const ready = path.trim() !== '' && password !== '' && confirm === password
  const reveal = platform === 'macos' ? '在 Finder 中顯示' : '在資料夾中顯示'

  return (
    <DialogContent className="sm:max-w-md" {...busyProps(running)}>
      <form
        className="flex flex-col gap-4"
        onSubmit={(event) => {
          event.preventDefault()
          if (!ready || running) return
          setStopped(false)
          setRunning(true)
        }}
      >
        <DialogHeader>
          <DialogTitle>把 PathKeep 打包成一個檔案</DialogTitle>
          <DialogDescription>檔案包含你的歷史、每筆記錄的來源、Ask 對話和設定，並用你在這裡設定的密碼加密。搜尋和智慧分析會在另一台電腦上重建。</DialogDescription>
        </DialogHeader>
        <PathField id="move-export-path" label="儲存到" value={path} picked={pathIn(platform, paths.desktop, name)} onChange={setPath} disabled={running} />
        <div className="flex flex-col gap-1.5">
          <Label htmlFor="move-export-password">檔案密碼</Label>
          <Input id="move-export-password" type="password" autoComplete="new-password" value={password} disabled={running} onChange={(event) => setPassword(event.target.value)} />
        </div>
        <div className="flex flex-col gap-1.5">
          <Label htmlFor="move-export-password-confirm">再輸入一次</Label>
          <Input
            id="move-export-password-confirm"
            type="password"
            autoComplete="new-password"
            value={confirm}
            disabled={running}
            aria-invalid={differ}
            aria-describedby="move-export-password-hint"
            onChange={(event) => setConfirm(event.target.value)}
          />
          <p id="move-export-password-hint" className={differ ? 'text-[13px] text-destructive' : 'text-[13px] text-muted-foreground'}>
            {differ ? '兩次輸入的密碼不一致。' : '在另一台電腦上開啟檔案時需要它，之後它就是那邊存檔的密碼。忘記了就無法找回。'}
          </p>
        </div>
        <p className="text-[13px] text-muted-foreground">App 密碼和備份排程會留在這台電腦上。</p>
        {running && (
          <MaintenanceProgress
            steps={operationSteps.export}
            onDone={() => {
              setRunning(false)
              toast.success(`已儲存 ${size(3_318_000_000)}。`, {
                description: path.trim(),
                action: { label: reveal, onClick: () => toast('原型：這裡會在檔案管理員裡選中這個檔案。') },
              })
              onClose()
            }}
            onStopped={() => {
              setRunning(false)
              setStopped(true)
            }}
          />
        )}
        {stopped && <StoppedLine />}
        <DialogFooter>
          <Button type="button" variant="ghost" disabled={running} onClick={onClose}>
            取消
          </Button>
          <Button type="submit" disabled={!ready || running}>
            匯出
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  )
}
