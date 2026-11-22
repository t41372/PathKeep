/**
 * A file path the user types or picks. Mirrors
 * `src/features/settings/path-field.tsx`; in the desktop app 選擇… opens the
 * system's save or open panel, here it fills in `picked`.
 */
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'

export const BUNDLE_EXTENSION = 'pathkeep'

export function PathField({ id, label, value, picked, onChange, disabled }: { id: string; label: string; value: string; picked: string; onChange: (path: string) => void; disabled?: boolean }) {
  return (
    <div className="flex flex-col gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      <div className="flex gap-2">
        <Input id={id} value={value} disabled={disabled} spellCheck={false} className="font-mono text-xs" onChange={(event) => onChange(event.target.value)} />
        <Button type="button" variant="outline" disabled={disabled} onClick={() => onChange(picked)}>
          選擇…
        </Button>
      </div>
    </div>
  )
}
