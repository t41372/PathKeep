/** `value`, once it has not changed for `ms`: a card's search box. */
import { useEffect, useState } from 'react'

export function useDebounced<T>(value: T, ms = 200) {
  const [settled, setSettled] = useState(value)
  useEffect(() => {
    const timer = setTimeout(() => setSettled(value), ms)
    return () => clearTimeout(timer)
  }, [value, ms])
  return settled
}
