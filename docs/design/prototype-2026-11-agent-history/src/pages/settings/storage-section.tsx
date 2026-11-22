/**
 * Settings → 儲存. Mirrors `src/features/settings/storage-section.tsx`:
 * where the archive lives, disk usage, freeing space, and the 搬移與還原
 * group (move to another computer, restore from a safety copy, delete all
 * data). Each destructive action opens a dialog that shows what will happen
 * before it runs.
 *
 * Design changes: 連結預覽圖 and 匯出歷史 moved to Settings › 瀏覽紀錄;
 * 存檔大小 (one line per data type) sits under disk usage.
 */
import { useState } from 'react'
import { toast } from 'sonner'
import { SettingRow, SettingsGroup, SettingsSection } from '@/components/app/setting-row'
import { Button } from '@/components/ui/button'
import { appPaths, usePlatform } from './pathkeep-kit/platform'
import { PlatformSelect, ProtoBar, ProtoHint } from './pathkeep-kit/proto-bar'
import { initialBuckets, recoverySnapshots, type BucketId } from './storage/data'
import { FreeSpaceDialog } from './storage/free-space-dialog'
import { MoveDialog } from './storage/move-dialogs'
import { RestoreDialog } from './storage/restore-dialog'
import { ArchiveSizeRow, UsageCard } from './storage/usage-card'
import { WipeDialog } from './storage/wipe-dialog'

type Open = 'free' | 'export' | 'import' | 'restore' | 'wipe' | null

export function StorageSection() {
  const [open, setOpen] = useState<Open>(null)
  const [buckets, setBuckets] = useState(initialBuckets)
  const close = () => setOpen(null)
  // Deleting the snapshots deletes the safety copies too.
  const snapshots = buckets.snapshots > 0 ? recoverySnapshots : []

  const pruned = (ids: BucketId[]) =>
    setBuckets((current) => {
      const next = { ...current }
      for (const id of ids) next[id] = 0
      return next
    })

  return (
    <SettingsSection title="儲存">
      <ProtoBar>
        <PlatformSelect />
        <ProtoHint>還原和開啟檔案時，密碼打 wrong 就算打錯</ProtoHint>
      </ProtoBar>
      <LocationRow />
      <UsageCard buckets={buckets} />
      <ArchiveSizeRow />
      <SettingRow
        title="釋放空間"
        description="刪除舊快照、匯出檔案和殘留檔案。你的歷史不受影響。"
        control={
          <Button size="sm" variant="outline" onClick={() => setOpen('free')}>
            檢視…
          </Button>
        }
      />

      <SettingsGroup title="搬移與還原" />
      <SettingRow
        title="搬到另一台電腦"
        description="把所有內容打包成一個檔案，或開啟在另一台電腦上製作的檔案。"
        control={
          <>
            <Button size="sm" variant="outline" onClick={() => setOpen('import')}>
              匯入…
            </Button>
            <Button size="sm" variant="outline" onClick={() => setOpen('export')}>
              匯出…
            </Button>
          </>
        }
      />
      <SettingRow
        title="從安全副本還原"
        description="在更改密碼等有風險的操作之前，PathKeep 會先保存一份副本"
        control={
          <Button size="sm" variant="outline" onClick={() => setOpen('restore')}>
            選擇…
          </Button>
        }
      />
      <SettingRow
        danger
        title="刪除所有資料"
        description="存檔、索引、快照和匯出檔案都會被刪除，無法復原。"
        control={
          <Button size="sm" variant="destructive" onClick={() => setOpen('wipe')}>
            刪除…
          </Button>
        }
      />

      <FreeSpaceDialog open={open === 'free'} buckets={buckets} onClose={close} onPruned={pruned} />
      <MoveDialog mode={open === 'export' || open === 'import' ? open : null} onClose={close} />
      <RestoreDialog open={open === 'restore'} snapshots={snapshots} onClose={close} />
      <WipeDialog open={open === 'wipe'} buckets={buckets} onClose={close} />
    </SettingsSection>
  )
}

function LocationRow() {
  const platform = usePlatform()
  const root = appPaths(platform).root
  return (
    <SettingRow
      title="存檔位置"
      description={<span className="font-mono text-xs break-all">{root}</span>}
      control={
        <Button size="sm" variant="outline" onClick={() => toast(platform === 'macos' ? '原型：這裡會在 Finder 打開這個資料夾。' : '原型：這裡會在檔案管理員打開這個資料夾。')}>
          {platform === 'macos' ? '在 Finder 中顯示' : '在資料夾中顯示'}
        </Button>
      }
    />
  )
}
