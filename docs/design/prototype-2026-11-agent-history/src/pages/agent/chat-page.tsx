/** A conversation on its own page, for long reads (outline and info open by default). */
import { useNavigate, useParams } from 'react-router-dom'
import { archive } from '~/data/archive'
import { ChatDialogs } from './chats/chat-actions'
import { useChatEdits } from './chats/chat-edits'
import { Reader } from './reader/reader'

export default function ChatPage() {
  const { chatId } = useParams()
  const navigate = useNavigate()
  const { deleted } = useChatEdits()
  const chat = archive.chatById.get(Number(chatId))
  if (!chat || deleted.has(chat.id)) return <div className="flex flex-1 items-center justify-center text-muted-foreground">找不到這段對話</div>
  return (
    <>
      <Reader key={chat.id} chat={chat} page onClose={() => void navigate(-1)} />
      <ChatDialogs onDeleted={() => void navigate('/agent/chats', { replace: true })} />
    </>
  )
}
