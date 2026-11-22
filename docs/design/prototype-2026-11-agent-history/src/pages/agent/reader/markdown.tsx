/** The agent's markdown, typeset like Ask's answers (same Streamdown setup). */
import { memo } from 'react'
import { Streamdown } from 'streamdown'
import { cn } from '@/lib/cn'

const prose = cn(
  'text-[14.5px] leading-[1.65] break-words',
  '[&_p]:my-2 [&_p:first-child]:mt-0 [&_p:last-child]:mb-0',
  '[&_ul]:my-2 [&_ul]:list-disc [&_ul]:pl-5 [&_ol]:my-2 [&_ol]:list-decimal [&_ol]:pl-5 [&_li]:my-0.5',
  '[&_h1]:mt-4 [&_h1]:mb-2 [&_h1]:text-lg [&_h1]:font-semibold',
  '[&_h2]:mt-4 [&_h2]:mb-2 [&_h2]:text-base [&_h2]:font-semibold',
  '[&_a]:text-brand [&_a]:underline [&_a]:underline-offset-2',
  '[&_code]:rounded [&_code]:bg-muted [&_code]:px-1 [&_code]:py-0.5 [&_code]:font-mono [&_code]:text-[12.5px]',
  '[&_pre]:my-2 [&_pre]:overflow-x-auto [&_pre]:rounded-lg [&_pre]:bg-muted [&_pre]:p-3',
  '[&_pre_code]:bg-transparent [&_pre_code]:p-0',
  '[&_table]:my-2 [&_table]:w-full [&_table]:border-collapse [&_table]:text-[13px]',
  '[&_th]:border-b [&_th]:px-2 [&_th]:py-1.5 [&_th]:text-left [&_th]:font-medium',
  '[&_td]:border-b [&_td]:px-2 [&_td]:py-1.5',
)

export const Markdown = memo(function Markdown({ content }: { content: string }) {
  return (
    <div className={prose}>
      <Streamdown mode="static" controls={false} lineNumbers={false} linkSafety={{ enabled: false }}>
        {content}
      </Streamdown>
    </div>
  )
})
