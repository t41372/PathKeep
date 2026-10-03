/**
 * Renders the assistant's markdown while it streams.
 *
 * Streamdown's own utility classes are not part of our Tailwind scan, so the
 * typography is set here. Links open in the system browser and code has no
 * highlighter (no extra dependency for something a history answer rarely has).
 */
import { memo, type ComponentProps } from 'react'
import { Streamdown, type Components } from 'streamdown'
import { supportClient } from '@/lib/backend-client/support'
import { cn } from '@/lib/cn'

const prose = cn(
  'text-[15px] leading-[1.65] break-words',
  '[&_p]:my-2 [&_p:first-child]:mt-0 [&_p:last-child]:mb-0',
  '[&_ul]:my-2 [&_ul]:list-disc [&_ul]:pl-5 [&_ol]:my-2 [&_ol]:list-decimal [&_ol]:pl-5 [&_li]:my-0.5',
  '[&_h1]:mt-4 [&_h1]:mb-2 [&_h1]:text-lg [&_h1]:font-semibold',
  '[&_h2]:mt-4 [&_h2]:mb-2 [&_h2]:text-base [&_h2]:font-semibold',
  '[&_h3]:mt-3 [&_h3]:mb-1.5 [&_h3]:font-semibold',
  '[&_a]:text-brand [&_a]:underline [&_a]:underline-offset-2',
  '[&_blockquote]:my-2 [&_blockquote]:border-l-2 [&_blockquote]:pl-3 [&_blockquote]:text-muted-foreground',
  '[&_code]:rounded [&_code]:bg-muted [&_code]:px-1 [&_code]:py-0.5 [&_code]:font-mono [&_code]:text-[13px]',
  '[&_pre]:my-2 [&_pre]:overflow-x-auto [&_pre]:rounded-lg [&_pre]:bg-muted [&_pre]:p-3',
  '[&_pre_code]:bg-transparent [&_pre_code]:p-0',
  '[&_table]:my-2 [&_table]:w-full [&_table]:border-collapse [&_table]:text-[13px]',
  '[&_th]:border-b [&_th]:px-2 [&_th]:py-1.5 [&_th]:text-left [&_th]:font-medium',
  '[&_td]:border-b [&_td]:px-2 [&_td]:py-1.5',
)

function ExternalLink({ href, children }: ComponentProps<'a'>) {
  return (
    <a
      href={href}
      onClick={(event) => {
        event.preventDefault()
        if (href)
          void supportClient.openExternalUrl(href).catch(() => undefined)
      }}
    >
      {children}
    </a>
  )
}

// Streamdown types `components` with an index signature our component cannot satisfy.
const components = { a: ExternalLink } as Components

export const Markdown = memo(function Markdown({
  content,
  streaming,
}: {
  content: string
  streaming: boolean
}) {
  return (
    <div className={prose}>
      <Streamdown
        mode={streaming ? 'streaming' : 'static'}
        controls={false}
        lineNumbers={false}
        linkSafety={{ enabled: false }}
        components={components}
      >
        {content}
      </Streamdown>
    </div>
  )
})
