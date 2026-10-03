/**
 * A file path the user types or picks with the native dialog. The backend
 * needs a real path; in the desktop app "Choose…" opens the OS picker, and in
 * a plain browser (dev bridge) the path is typed.
 */
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useI18n } from '@/lib/i18n'
import { hasTauriGuestApi } from '@/lib/runtime'

export const BUNDLE_EXTENSION = 'pathkeep'

async function pick(mode: 'save' | 'open', defaultPath?: string) {
  const dialog = await import('@tauri-apps/plugin-dialog')
  const filters = [{ name: 'PathKeep', extensions: [BUNDLE_EXTENSION] }]
  const picked =
    mode === 'save'
      ? await dialog.save({ defaultPath, filters })
      : await dialog.open({ multiple: false, directory: false, filters })
  return typeof picked === 'string' ? picked : null
}

export function PathField({
  id,
  label,
  mode,
  value,
  onChange,
  disabled,
}: {
  id: string
  label: string
  mode: 'save' | 'open'
  value: string
  onChange: (path: string) => void
  disabled?: boolean
}) {
  const { t } = useI18n()
  return (
    <div className="flex flex-col gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      <div className="flex gap-2">
        <Input
          id={id}
          value={value}
          disabled={disabled}
          spellCheck={false}
          className="font-mono text-xs"
          onChange={(event) => onChange(event.target.value)}
        />
        {hasTauriGuestApi() && (
          <Button
            type="button"
            variant="outline"
            disabled={disabled}
            onClick={() =>
              void pick(mode, value || undefined).then(
                (path) => path && onChange(path),
              )
            }
          >
            {t('settingsStorage.move.choose')}
          </Button>
        )}
      </div>
    </div>
  )
}
