/**
 * What a prompt carried (`agent-history.md` §4.5): an image shows a
 * thumbnail, read from the archive when it scrolls into view; another
 * file shows a chip with its name; one PathKeep does not have is a dashed
 * chip that says why.
 */
import { useQuery } from '@tanstack/react-query'
import { File, ImageIcon, ImageOff } from 'lucide-react'
import { useEffect, useState } from 'react'
import {
  agentChatsClient,
  type ChatAttachment,
} from '@/lib/backend-client/agent-chats'
import { useFormat, useI18n, type MessageKey } from '@/lib/i18n'

const PAGE = 65_536
/** Larger images stay a chip: reading them would hold the reader up. */
const THUMBNAIL_LIMIT = 16 * 1024 * 1024

const reasons: Record<string, MessageKey> = {
  missing: 'agentReader.attachment.missing.missing',
  not_found: 'agentReader.attachment.missing.missing',
  remote: 'agentReader.attachment.missing.remote',
  remote_unavailable: 'agentReader.attachment.missing.remote',
  outside: 'agentReader.attachment.missing.outside',
  outside_roots: 'agentReader.attachment.missing.outside',
  denied: 'agentReader.attachment.missing.denied',
  unsupported: 'agentReader.attachment.missing.unsupported',
  unresolved: 'agentReader.attachment.missing.notInRecord',
}

function isImage(attachment: ChatAttachment) {
  return (attachment.mimeType ?? attachment.kind ?? '').startsWith('image')
}

/** Every page of an attachment's bytes, as one Blob. */
async function readAttachment(
  id: string,
  publication: string,
  attachment: ChatAttachment,
) {
  const chunks: Uint8Array[] = []
  let offset: number | null = 0
  while (offset !== null) {
    const page = await agentChatsClient.attachment({
      id,
      publication,
      sourceId: attachment.sourceId,
      ordinal: attachment.ordinal,
      key: attachment.key,
      offset,
      limit: PAGE,
    })
    chunks.push(Uint8Array.from(page.bytes))
    offset = page.next
  }
  return new Blob(chunks as BlobPart[], {
    type: attachment.mimeType ?? 'application/octet-stream',
  })
}

function Thumbnail({
  id,
  publication,
  attachment,
}: {
  id: string
  publication: string
  attachment: ChatAttachment
}) {
  const { t } = useI18n()
  const blob = useQuery({
    queryKey: [
      'archive',
      'agent-chats',
      'chat',
      id,
      'attachment',
      publication,
      attachment.sourceId,
      attachment.ordinal,
      attachment.key,
    ],
    queryFn: () => readAttachment(id, publication, attachment),
    staleTime: Infinity,
  })
  // An object URL is a resource: made by the effect that shows it and
  // revoked by its cleanup, so a remount (StrictMode) makes a new one.
  const [url, setUrl] = useState<string | null>(null)
  useEffect(() => {
    if (!blob.data) return
    const next = URL.createObjectURL(blob.data)
    // eslint-disable-next-line react-hooks/set-state-in-effect -- the URL exists only from here to the cleanup
    setUrl(next)
    return () => URL.revokeObjectURL(next)
  }, [blob.data])
  if (blob.isError)
    return (
      <span className="flex items-center gap-1.5 rounded-lg border border-dashed px-2.5 py-1.5 text-[12px] text-muted-foreground">
        <ImageOff className="size-3.5" aria-hidden />
        {t('agentReader.attachment.imageFailed')}
      </span>
    )
  return (
    <span
      className="flex h-24 min-w-24 items-center justify-center overflow-hidden rounded-lg border bg-muted"
      role={url ? undefined : 'status'}
      aria-label={url ? undefined : t('agentReader.attachment.loadingImage')}
    >
      {url ? (
        <img
          src={url}
          alt={attachment.name ?? t('agentReader.attachment.image')}
          className="animate-rise h-24 w-auto max-w-[240px] object-contain"
        />
      ) : (
        <ImageIcon className="size-5 text-muted-foreground/60" aria-hidden />
      )}
    </span>
  )
}

export function Attachments({
  id,
  publication,
  attachments,
}: {
  id: string
  publication: string
  attachments: ChatAttachment[]
}) {
  const { t } = useI18n()
  const f = useFormat()
  if (attachments.length === 0) return null
  return (
    <div className="flex max-w-[85%] flex-wrap justify-end gap-1.5">
      {attachments.map((attachment) => {
        const key = `${attachment.sourceId}:${attachment.ordinal}:${attachment.key}`
        const name =
          attachment.name ??
          t(
            isImage(attachment)
              ? 'agentReader.attachment.image'
              : 'agentReader.attachment.file',
          )
        if (attachment.availability === 'missing') {
          const reason = attachment.reason ?? ''
          return (
            <span
              key={key}
              className="flex items-center gap-1.5 rounded-lg border border-dashed px-2.5 py-1.5 text-[12px] text-muted-foreground"
            >
              <ImageOff className="size-3.5 shrink-0" aria-hidden />
              <span>
                {name}
                <span className="text-foreground/70">
                  {' · '}
                  {reasons[reason]
                    ? t(reasons[reason])
                    : t('agentReader.attachment.missing.other', { reason })}
                </span>
              </span>
            </span>
          )
        }
        if (isImage(attachment) && (attachment.bytes ?? 0) <= THUMBNAIL_LIMIT)
          return (
            <Thumbnail
              key={key}
              id={id}
              publication={publication}
              attachment={attachment}
            />
          )
        return (
          <span
            key={key}
            className="flex items-center gap-1.5 rounded-lg border px-2.5 py-1.5 text-[12px]"
          >
            <File
              className="size-3.5 shrink-0 text-muted-foreground"
              aria-hidden
            />
            {name}
            {attachment.bytes != null && (
              <span className="text-muted-foreground tabular">
                {f.bytes(attachment.bytes)}
              </span>
            )}
          </span>
        )
      })}
    </div>
  )
}
