/**
 * Rebuild search index: the row and the confirmation before rebuilding the
 * chosen model's index from nothing. Mirrors RebuildRow in
 * src/features/settings/ai-section.tsx, rebuild-dialog.tsx and use-rebuild.ts.
 */
import { useEffect, useState } from 'react'
import { toast } from 'sonner'
import { SettingRow } from '@/components/app/setting-row'
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { Skeleton } from '@/components/ui/skeleton'
import { num } from '~/data/format'
import { TOTAL_VISITS, useAi } from './state'

export function RebuildDialog({ open, onOpenChange, onConfirm }: { open: boolean; onOpenChange: (open: boolean) => void; onConfirm: () => void }) {
  // The estimate is read when the dialog opens.
  const [counted, setCounted] = useState(false)
  useEffect(() => {
    if (!open) return
    const timer = window.setTimeout(() => setCounted(true), 500)
    return () => {
      window.clearTimeout(timer)
      setCounted(false)
    }
  }, [open])

  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>要重建搜尋索引嗎？</AlertDialogTitle>
          {!counted ? (
            <Skeleton className="h-10 w-full" />
          ) : (
            <AlertDialogDescription>
              這會在背景重新索引 {num(TOTAL_VISITS)} 次造訪，期間可以照常使用 PathKeep，進度顯示在這裡。新索引建好之前，搜尋繼續用現有的索引，所以這段時間磁碟上會同時有兩份。你的歷史不會被變更。
            </AlertDialogDescription>
          )}
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>取消</AlertDialogCancel>
          <AlertDialogAction onClick={onConfirm}>重建</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}

export function RebuildRow() {
  const ai = useAi()
  const [open, setOpen] = useState(false)
  const busy = ai.phase === 'running' || ai.phase === 'queued' || ai.phase.startsWith('paused') || ai.phase === 'setupRunning'
  return (
    <SettingRow
      title="重建搜尋索引"
      description={ai.enabled ? '重新讀取你的全部歷史並重建語意索引。期間可以照常使用 PathKeep。' : '請先開啟本機語意搜尋。'}
      control={
        <Button size="sm" variant="outline" disabled={!ai.enabled || busy} onClick={() => setOpen(true)}>
          重建
        </Button>
      }
    >
      <RebuildDialog
        open={open}
        onOpenChange={setOpen}
        onConfirm={() => {
          ai.set({ phase: 'running', done: 0 })
          toast.success('正在重建搜尋索引。')
        }}
      />
    </SettingRow>
  )
}
