/**
 * Asks for the archive password so it can go into the system keychain.
 * Mirrors `src/features/settings/keychain-dialog.tsx`; saving takes a moment
 * and then turns the switch on.
 */
import { useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Spinner } from '@/components/ui/spinner'

export function KeychainDialog({ open, onClose, onSaved }: { open: boolean; onClose: () => void; onSaved: () => void }) {
  return (
    <Dialog open={open} onOpenChange={(next) => !next && onClose()}>
      {open && <KeychainForm onClose={onClose} onSaved={onSaved} />}
    </Dialog>
  )
}

function KeychainForm({ onClose, onSaved }: { onClose: () => void; onSaved: () => void }) {
  const [password, setPassword] = useState('')
  const [saving, setSaving] = useState(false)
  return (
    <DialogContent className="sm:max-w-sm">
      <form
        className="flex flex-col gap-5"
        onSubmit={(event) => {
          event.preventDefault()
          if (!password || saving) return
          setSaving(true)
          setTimeout(() => {
            toast.success('密碼已存到鑰匙圈。')
            onSaved()
            onClose()
          }, 500)
        }}
      >
        <DialogHeader>
          <DialogTitle>把密碼存到鑰匙圈</DialogTitle>
          <DialogDescription>輸入你用來解鎖存檔的密碼，它會儲存到系統鑰匙圈。</DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-1.5">
          <Label htmlFor="keychain-password">存檔密碼</Label>
          <Input id="keychain-password" type="password" autoComplete="current-password" autoFocus value={password} disabled={saving} onChange={(event) => setPassword(event.target.value)} />
        </div>
        <DialogFooter>
          <Button type="button" variant="ghost" disabled={saving} onClick={onClose}>
            取消
          </Button>
          <Button type="submit" disabled={!password || saving}>
            {saving && <Spinner />}
            儲存
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  )
}
