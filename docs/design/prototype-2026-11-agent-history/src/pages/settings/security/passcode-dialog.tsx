/**
 * Set or change the App Lock passcode, with the optional hint. Mirrors
 * `src/features/settings/passcode-dialog.tsx`: setting one for the first
 * time also turns App Lock on; changing it starts from the saved hint.
 */
import { useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Spinner } from '@/components/ui/spinner'
import { emptySecret, NewSecretFields, secretProblem, type NewSecret } from './new-secret-fields'

/** The backend refuses anything shorter. */
const PASSCODE_MIN_LENGTH = 4

export function PasscodeDialog({ mode, savedHint, onClose, onSaved }: { mode: 'set' | 'change' | null; savedHint: string; onClose: () => void; onSaved: (hint: string) => void }) {
  return (
    <Dialog open={mode !== null} onOpenChange={(open) => !open && onClose()}>
      {mode && <PasscodeForm key={mode} mode={mode} savedHint={savedHint} onClose={onClose} onSaved={onSaved} />}
    </Dialog>
  )
}

function PasscodeForm({ mode, savedHint, onClose, onSaved }: { mode: 'set' | 'change'; savedHint: string; onClose: () => void; onSaved: (hint: string) => void }) {
  const [secret, setSecret] = useState<NewSecret>(emptySecret)
  const [hint, setHint] = useState(mode === 'change' ? savedHint : '')
  const [saving, setSaving] = useState(false)
  const ready = secretProblem(secret, PASSCODE_MIN_LENGTH) === null

  return (
    <DialogContent className="sm:max-w-sm">
      <form
        className="flex flex-col gap-5"
        onSubmit={(event) => {
          event.preventDefault()
          if (!ready || saving) return
          setSaving(true)
          setTimeout(() => {
            toast.success(mode === 'set' ? 'App 密碼已儲存，App 鎖定已開啟。' : 'App 密碼已更改。')
            onSaved(hint.trim())
            onClose()
          }, 500)
        }}
      >
        <DialogHeader>
          <DialogTitle>{mode === 'set' ? '設定 App 密碼' : '更改 App 密碼'}</DialogTitle>
          <DialogDescription>解鎖 PathKeep 時輸入。它不是你的存檔密碼。</DialogDescription>
        </DialogHeader>
        <NewSecretFields idPrefix="passcode" label="App 密碼" secret={secret} minLength={PASSCODE_MIN_LENGTH} onChange={setSecret} disabled={saving} />
        <div className="flex flex-col gap-1.5">
          <Label htmlFor="passcode-hint">提示（選填）</Label>
          <Input id="passcode-hint" value={hint} autoComplete="off" disabled={saving} onChange={(event) => setHint(event.target.value)} />
          <p className="text-[13px] text-muted-foreground">忘記 App 密碼時會顯示在鎖定畫面上。這台電腦前的任何人都看得到，別直接寫下密碼。</p>
        </div>
        <DialogFooter>
          <Button type="button" variant="ghost" disabled={saving} onClick={onClose}>
            取消
          </Button>
          <Button type="submit" disabled={!ready || saving}>
            {saving && <Spinner />}
            儲存密碼
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  )
}
