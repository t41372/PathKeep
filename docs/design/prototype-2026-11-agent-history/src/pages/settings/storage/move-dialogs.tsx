/**
 * 搬到另一台電腦: pack everything into one `.pathkeep` file, or open such a
 * file here. Mirrors `src/features/settings/move-dialogs.tsx`; the two forms
 * are in `export-form.tsx` and `import-form.tsx`.
 */
import { Dialog } from '@/components/ui/dialog'
import { ExportForm } from './export-form'
import { ImportForm } from './import-form'

export function MoveDialog({ mode, onClose }: { mode: 'export' | 'import' | null; onClose: () => void }) {
  return (
    <Dialog open={mode !== null} onOpenChange={(open) => !open && onClose()}>
      {mode === 'export' && <ExportForm onClose={onClose} />}
      {mode === 'import' && <ImportForm onClose={onClose} />}
    </Dialog>
  )
}

/** While a long run goes on the dialog cannot be closed. */
export function busyProps(busy: boolean) {
  return {
    showCloseButton: !busy,
    onInteractOutside: (event: Event) => busy && event.preventDefault(),
    onEscapeKeyDown: (event: KeyboardEvent) => busy && event.preventDefault(),
  }
}
