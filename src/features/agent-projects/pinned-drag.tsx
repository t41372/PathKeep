/**
 * Dragging in the default view (`agent-history.md` §5.2): a pin or a
 * section by its grip, and a project from Other projects into Pinned. The
 * drop lands before, after or into the row under the pointer (a section's
 * top and bottom edges are before and after, its middle is into; a
 * project's halves are before and after); on Pinned's empty space it goes
 * last at the top level; on Other projects it unpins (projects only). A
 * collapsed section held over for a moment opens. A line shows where it
 * lands; a label follows the pointer and says when it cannot land or will
 * unpin. Esc cancels.
 *
 * Built on dnd-kit's pointer and keyboard sensors, not HTML5 drag events:
 * the desktop app takes file drops natively, and with that on, Windows'
 * webview does not hand HTML5 drag events to the page. Every move here can
 * also be made from the menus.
 */
import { DragDropProvider, DragOverlay, useDroppable } from '@dnd-kit/react'
import { Check } from 'lucide-react'
import { useRef, useState, type ReactNode } from 'react'
import type { ProjectAction } from '@/lib/backend-client/agent-projects'
import { cn } from '@/lib/cn'
import { useI18n } from '@/lib/i18n'
import { moveFor, type Arrangement, type DropZone } from './arrangement'
import { projectName } from './project-words'
import { useProjectsShared } from './projects-context'
import {
  AREA_PRIORITY,
  DragContext,
  ROW_PRIORITY,
  useDragState,
  type Dragged,
  type DropHint,
} from './use-pinned-drag'
import { useProjectEdit } from './use-project-edit'

const OPEN_AFTER_MS = 650

function zoneAt(element: Element, y: number, section: boolean): DropZone {
  const rect = element.getBoundingClientRect()
  const at = (y - rect.top) / Math.max(1, rect.height)
  if (!section) return at < 0.5 ? 'before' : 'after'
  if (at < 0.28) return 'before'
  if (at > 0.72) return 'after'
  return 'into'
}

/** The drop's meaning; `null` when it cannot land there. */
function actionFor(
  arrangement: Arrangement,
  id: string,
  kind: 'project' | 'section',
  hint: DropHint,
): Extract<ProjectAction, { kind: 'move' | 'unpin' }> | null {
  if (hint.zone === 'other')
    return kind === 'project' && arrangement.pinned.has(id)
      ? { kind: 'unpin', id }
      : null
  if (hint.zone === 'end') return { kind: 'move', id, parent: null }
  if (hint.over.startsWith('into:'))
    return moveFor(arrangement, id, hint.over.slice(5), 'into')
  return moveFor(arrangement, id, hint.over, hint.zone)
}

/** Where `moving` would land for the drag as it is now; `null` over nothing. */
function hintAt(
  arrangement: Arrangement,
  moving: Dragged,
  operation: {
    target: { data: unknown; element?: Element | null } | null
    position: { current: { y: number } }
  },
): DropHint | null {
  const { target, position } = operation
  if (!target) return null
  const data = target.data as {
    over: string
    section?: boolean
    zone?: 'end' | 'other'
  }
  let next: DropHint
  if (data.zone) next = { over: data.over, zone: data.zone, ok: true }
  else if (data.over.startsWith('into:'))
    next = { over: data.over, zone: 'into', ok: true }
  else {
    const element = target.element
    const zone = element
      ? zoneAt(element, position.current.y, Boolean(data.section))
      : 'after'
    next = { over: data.over, zone, ok: true }
  }
  next.ok = actionFor(arrangement, moving.id, moving.kind, next) !== null
  return next
}

export function PinnedDrag({ children }: { children: ReactNode }) {
  const { t } = useI18n()
  const shared = useProjectsShared()
  const edit = useProjectEdit()
  const [dragging, setDraggingState] = useState<Dragged | null>(null)
  const [hint, setHintState] = useState<DropHint | null>(null)
  // The handlers read these: they may be the ones from an earlier render.
  const draggingRef = useRef<Dragged | null>(null)
  const hintRef = useRef<DropHint | null>(null)
  const setDragging = (next: Dragged | null) => {
    draggingRef.current = next
    setDraggingState(next)
  }
  const setHint = (next: DropHint | null) => {
    const previous = hintRef.current
    if (
      previous === next ||
      (previous &&
        next &&
        previous.over === next.over &&
        previous.zone === next.zone &&
        previous.ok === next.ok)
    )
      return
    hintRef.current = next
    setHintState(next)
  }
  const dropped = useRef(0)
  const opener = useRef<{
    id: string
    timer: ReturnType<typeof setTimeout>
  } | null>(null)
  const { arrangement, cards } = shared

  const stopOpener = () => {
    if (opener.current) clearTimeout(opener.current.timer)
    opener.current = null
  }
  const labelOf = (id: string, kind: 'project' | 'section') => {
    if (kind === 'section') return arrangement.byId.get(id)?.node.name ?? ''
    const card = cards.get(id)
    return card ? projectName(t, card) : id
  }

  return (
    <DragContext.Provider
      value={{
        dragging: dragging?.id ?? null,
        hint,
        justDropped: () => Date.now() - dropped.current < 250,
      }}
    >
      <DragDropProvider
        onDragStart={(event) => {
          const source = event.operation.source
          if (!source) return
          const data = source.data as { kind: 'project' | 'section' }
          setDragging({ id: String(source.id), kind: data.kind })
        }}
        onDragMove={(event) => {
          const moving = draggingRef.current
          if (!moving) return
          const next = hintAt(arrangement, moving, event.operation)
          setHint(next)
          if (!next) {
            stopOpener()
            return
          }
          // Held over a collapsed section: open it.
          const section = arrangement.byId.get(next.over)?.node
          if (
            section?.kind === 'section' &&
            section.collapsed &&
            next.zone === 'into'
          ) {
            if (opener.current?.id !== section.id) {
              stopOpener()
              opener.current = {
                id: section.id,
                timer: setTimeout(() => {
                  opener.current = null
                  void edit({
                    kind: 'collapse',
                    id: section.id,
                    collapsed: false,
                  })
                }, OPEN_AFTER_MS),
              }
            }
          } else stopOpener()
        }}
        onDragEnd={(event) => {
          stopOpener()
          const current = draggingRef.current
          // Where the pointer let go, not only where it last moved.
          const landed = current
            ? (hintAt(arrangement, current, event.operation) ?? hintRef.current)
            : null
          setDragging(null)
          setHint(null)
          dropped.current = Date.now()
          if (event.canceled || !current || !landed) return
          const action = actionFor(
            arrangement,
            current.id,
            current.kind,
            landed,
          )
          if (!action) return
          const name = labelOf(current.id, current.kind)
          const where =
            action.kind === 'unpin'
              ? t('agentProjects.where.other')
              : action.parent === null
                ? t('agentProjects.where.top')
                : t('agentProjects.where.section', {
                    name: arrangement.byId.get(action.parent)?.node.name ?? '',
                  })
          void edit(action, {
            title: t('agentProjects.toast.moved', { name, where }),
          })
        }}
      >
        {children}
        <DragOverlay dropAnimation={null}>
          {dragging && (
            <div className="flex h-8 items-center gap-1.5 rounded-md border bg-popover px-2.5 text-[12.5px] shadow-lg">
              <span className="max-w-[240px] truncate font-medium">
                {labelOf(dragging.id, dragging.kind)}
              </span>
              {hint && !hint.ok ? (
                <span className="text-muted-foreground">
                  {t('agentProjects.drag.cannot')}
                </span>
              ) : hint?.zone === 'other' ? (
                <span className="text-muted-foreground">
                  {t('agentProjects.drag.unpin')}
                </span>
              ) : hint ? (
                <Check className="size-3.5 text-brand" aria-hidden />
              ) : null}
            </div>
          )}
        </DragOverlay>
      </DragDropProvider>
    </DragContext.Provider>
  )
}

/** Pinned's empty space, an empty section's placeholder, or Other projects. */
export function DropArea({
  id,
  over,
  zone,
  className,
  children,
}: {
  id: string
  over: string
  zone?: 'end' | 'other'
  className?: string
  children: ReactNode
}) {
  const { ref, isDropTarget } = useDroppable({
    id,
    data: { over, ...(zone ? { zone } : {}) },
    // An empty section's placeholder is as specific as a row.
    collisionPriority: zone ? AREA_PRIORITY : ROW_PRIORITY,
  })
  const { dragging } = useDragState()
  return (
    <div
      ref={ref}
      data-drop-area={id}
      className={cn(
        className,
        dragging && isDropTarget && 'ring-2 ring-brand/40',
      )}
    >
      {children}
    </div>
  )
}
