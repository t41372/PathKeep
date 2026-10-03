/**
 * Keyboard for the History lists: ↑/↓ move the selection, Enter opens the
 * selected page, Esc closes the detail panel. Ignored while typing or while a
 * menu or popover is open.
 */
import { useEffect, useRef } from 'react'

interface Options {
  /** The visits in display order. */
  ids: number[]
  selectedId: number | null
  onSelectIndex: (index: number) => void
  onOpen: () => void
  onClose: () => void
}

function isTyping(target: EventTarget | null) {
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable ||
      ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName))
  )
}

export function useListKeys(options: Options) {
  const latest = useRef(options)
  useEffect(() => {
    latest.current = options
  })

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (
        event.metaKey ||
        event.ctrlKey ||
        event.altKey ||
        isTyping(event.target)
      )
        return
      if (document.querySelector('[role="dialog"], [role="menu"]')) return
      const { ids, selectedId, onSelectIndex, onOpen, onClose } = latest.current

      if (event.key === 'Escape') {
        onClose()
      } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        if (ids.length === 0) return
        event.preventDefault()
        const current = selectedId === null ? -1 : ids.indexOf(selectedId)
        const step = event.key === 'ArrowDown' ? 1 : -1
        onSelectIndex(Math.min(ids.length - 1, Math.max(0, current + step)))
      } else if (event.key === 'Enter' && selectedId !== null) {
        // A focused row button would otherwise just re-select itself.
        const target = event.target
        const inList =
          target instanceof HTMLElement && target.closest('[role="listbox"]')
        if (target === document.body || inList) {
          event.preventDefault()
          onOpen()
        }
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [])
}
