/**
 * One conversation as a full page (`#/agent/chat/:id`, `agent-history.md`
 * §4): the reader with its outline open. Closing goes back to where it was
 * opened from; moving it to the trash goes to the list; its project goes
 * to the project's page.
 */
import { useCallback } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { useAgentChatsSync } from '@/lib/queries/agent-chats'
import { agentProjectPath, spacePath } from '@/lib/spaces'
import { Reader } from './reader'

export default function ChatPage() {
  const { id = '' } = useParams()
  const navigate = useNavigate()
  useAgentChatsSync()
  const close = useCallback(() => {
    // Opened by its address with nothing before it: the list is "back".
    if (window.history.length > 1) void navigate(-1)
    else void navigate(spacePath('agent', 'chats'))
  }, [navigate])
  return (
    <div className="flex min-h-0 min-w-0 flex-1">
      <Reader
        key={id}
        id={id}
        beside={false}
        onClose={close}
        onTrashed={() => void navigate(spacePath('agent', 'chats'))}
        onOpenProject={(project) => void navigate(agentProjectPath(project))}
      />
    </div>
  )
}
