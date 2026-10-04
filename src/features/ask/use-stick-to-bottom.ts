/**
 * Keeps a scroll container pinned to its bottom while content grows, until
 * the user scrolls up to read; scrolling back down re-pins it.
 */
import { useCallback, useEffect, useRef } from 'react'

const THRESHOLD = 64

export function useStickToBottom() {
  const scrollRef = useRef<HTMLDivElement>(null)
  const contentRef = useRef<HTMLDivElement>(null)
  const stuck = useRef(true)

  const scrollToBottom = useCallback(() => {
    const box = scrollRef.current
    if (box) box.scrollTop = box.scrollHeight
  }, [])

  /** For a new message or a freshly opened chat: always follow. */
  const pin = useCallback(() => {
    stuck.current = true
    scrollToBottom()
  }, [scrollToBottom])

  const onScroll = useCallback(() => {
    const box = scrollRef.current
    if (!box) return
    stuck.current =
      box.scrollHeight - box.scrollTop - box.clientHeight < THRESHOLD
  }, [])

  useEffect(() => {
    const content = contentRef.current
    if (!content) return
    const observer = new ResizeObserver(() => {
      if (stuck.current) scrollToBottom()
    })
    observer.observe(content)
    return () => observer.disconnect()
  }, [scrollToBottom])

  return { scrollRef, contentRef, onScroll, pin }
}
