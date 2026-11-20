/**
 * The pieces of dragging in the default view that rows use: what is being
 * dragged and where it would land (`PinnedDrag` provides it), a row's grip
 * and a row as a drop target. See `pinned-drag.tsx`.
 */
import { useDraggable, useDroppable } from '@dnd-kit/react'
import { GripVertical } from 'lucide-react'
import { createContext, useContext } from 'react'
import { useI18n } from '@/lib/i18n'
import type { DropZone } from './arrangement'

/** Rows win over the areas they sit in (dnd-kit's High and Low). */
export const ROW_PRIORITY = 3
export const AREA_PRIORITY = 1

export interface Dragged {
  id: string
  kind: 'project' | 'section'
}

/** Where the dragged thing would land. */
export interface DropHint {
  over: string
  zone: DropZone | 'end' | 'other'
  ok: boolean
}

export interface DragState {
  dragging: string | null
  hint: DropHint | null
  /** A drag ended just now: the click it ends with is not a click. */
  justDropped: () => boolean
}

export const DragContext = createContext<DragState>({
  dragging: null,
  hint: null,
  justDropped: () => false,
})

export function useDragState() {
  return useContext(DragContext)
}

/** A grip that drags `id`; the row stays where it is until dropped. */
export function useGrip(id: string, kind: 'project' | 'section') {
  const { t } = useI18n()
  const { ref, handleRef, isDragging } = useDraggable({
    id,
    data: { kind },
  })
  const grip = (
    <button
      ref={handleRef}
      type="button"
      tabIndex={-1}
      aria-hidden
      data-grip=""
      title={t('agentProjects.drag.grip')}
      className="flex size-4 shrink-0 cursor-grab touch-none items-center justify-center text-muted-foreground/60 opacity-0 group-hover:opacity-100 active:cursor-grabbing"
    >
      <GripVertical className="size-3.5" aria-hidden />
    </button>
  )
  return { ref, grip, isDragging }
}

/** A row things can be dropped on: before, after, or into a section. */
export function useDropRow(id: string, section: boolean) {
  const { ref, isDropTarget } = useDroppable({
    id: `row:${id}`,
    data: { over: id, section },
    // Over a row, the row decides, not the card around it.
    collisionPriority: ROW_PRIORITY,
  })
  return { ref, isDropTarget }
}
