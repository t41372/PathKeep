/** Browser names, each with its icon, for a short line such as "Chrome, Firefox". */
import { BrowserIcon } from '@/lib/browser-icons'

export function BrowserNames({ names }: { names: string[] }) {
  if (names.length === 0) return <>—</>
  return (
    <span className="inline-flex flex-wrap items-center gap-x-2 gap-y-0.5">
      {names.map((name) => (
        <span key={name} className="inline-flex items-center gap-1">
          <BrowserIcon browserName={name} decorative className="size-3.5" />
          {name}
        </span>
      ))}
    </span>
  )
}
