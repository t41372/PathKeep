/**
 * What a search's status line says aloud: at most once a second while it
 * runs, at once when it ends (`agent-history.md` §3.4). Shared by both
 * kinds of Records search.
 */
import { useEffect, useRef, useState } from 'react'

export function useAnnounced(text: string, final: boolean) {
  const [said, setSaid] = useState(text)
  const last = useRef(0)
  useEffect(() => {
    const wait = final ? 0 : Math.max(0, 1000 - (Date.now() - last.current))
    const timer = setTimeout(() => {
      last.current = Date.now()
      setSaid(text)
    }, wait)
    return () => clearTimeout(timer)
  }, [text, final])
  return said
}
