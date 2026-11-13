/** Small layout pieces shared by the settings sections. */
import type { ReactNode } from 'react'
import { cn } from '@/lib/cn'

/** A group of rows inside one section, with an optional line of context. */
export function Group({ title, intro, children }: { title: string; intro?: ReactNode; children: ReactNode }) {
  return (
    <div className="mt-5 flex flex-col gap-3 first:mt-0">
      <div className="flex flex-col gap-1">
        <h3 className="text-[15px] font-semibold">{title}</h3>
        {intro && <p className="text-[13px] leading-relaxed text-muted-foreground">{intro}</p>}
      </div>
      {children}
    </div>
  )
}

/** A quiet box for "not offered yet" and similar notes. */
export function Note({ children, className }: { children: ReactNode; className?: string }) {
  return <div className={cn('rounded-xl border border-dashed px-[18px] py-3.5 text-[13px] leading-relaxed text-muted-foreground', className)}>{children}</div>
}

export function Mono({ children }: { children: ReactNode }) {
  return <code className="rounded bg-muted px-1 py-0.5 font-mono text-[12px]">{children}</code>
}
