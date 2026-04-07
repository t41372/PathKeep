import { useState } from 'react'
import { useShellData } from '../../app/shell-data-context'
import { EmptyState } from '../../components/primitives/empty-state'
import { backend } from '../../lib/backend'
import type { AiAssistantCitation, AiAssistantResponse } from '../../lib/types'

interface Message {
  role: 'user' | 'ai'
  content: string
  citations?: AiAssistantCitation[]
}

const exampleConversation: Message[] = [
  {
    role: 'user',
    content:
      'When did I first start researching Tauri? What was the earliest thing I looked at?',
  },
  {
    role: 'ai',
    content:
      "Based on your browsing history, you first encountered Tauri on 2025-04-05, when you visited the Tauri GitHub repository.\n\nIn the following two weeks, you intensively read:\n- Tauri's official Getting Started and Architecture pages (14 visits total)\n- Multiple Tauri vs Electron comparison articles\n- Rust WebView2 binding related issues\n\nFrom May 2025, Tauri-related browsing frequency gradually increased, peaking in March 2026, which coincides with when you started developing PathKeep.",
    citations: [
      {
        historyId: 1,
        profileId: 'chrome:Default',
        url: 'https://github.com/tauri-apps/tauri',
        title: 'tauri-apps/tauri: Build desktop apps',
        visitedAt: '2025-04-05T00:00:00Z',
        score: 0.95,
      },
      {
        historyId: 2,
        profileId: 'chrome:Default',
        url: 'https://v2.tauri.app/start/',
        title: 'Getting Started | Tauri',
        visitedAt: '2025-04-06T00:00:00Z',
        score: 0.9,
      },
      {
        historyId: 3,
        profileId: 'chrome:Default',
        url: 'https://blog.example.com/tauri-vs-electron',
        title: 'Tauri vs Electron: Real-world Comparison',
        visitedAt: '2025-04-08T00:00:00Z',
        score: 0.85,
      },
      {
        historyId: 4,
        profileId: 'chrome:Default',
        url: 'https://v2.tauri.app/concepts/architecture/',
        title: 'Architecture | Tauri',
        visitedAt: '2025-04-12T00:00:00Z',
        score: 0.8,
      },
    ],
  },
]

export function AssistantPage() {
  const { snapshot } = useShellData()
  const [messages, setMessages] = useState<Message[]>(exampleConversation)
  const [input, setInput] = useState('')
  const [sending, setSending] = useState(false)

  async function handleSend() {
    const question = input.trim()
    if (!question || sending) return
    setInput('')
    setMessages((prev) => [...prev, { role: 'user', content: question }])
    setSending(true)
    try {
      const response: AiAssistantResponse = await backend.askAiAssistant({
        question,
      })
      setMessages((prev) => [
        ...prev,
        { role: 'ai', content: response.answer, citations: response.citations },
      ])
    } catch (e) {
      setMessages((prev) => [
        ...prev,
        {
          role: 'ai',
          content: `Error: ${e instanceof Error ? e.message : 'Failed to get response'}`,
        },
      ])
    } finally {
      setSending(false)
    }
  }

  if (!snapshot?.config.initialized) {
    return (
      <section className="page-shell">
        <EmptyState
          description="AI Assistant needs an initialized archive to search your browsing history."
          eyebrow="ASSISTANT"
          title="Archive not initialized"
        />
      </section>
    )
  }

  return (
    <section className="page-shell assistant-page" data-testid="assistant-page">
      <div className="assistant-container">
        <div className="assistant-messages">
          {messages.map((msg, i) => (
            <div
              key={i}
              className={`msg ${msg.role === 'user' ? 'msg-user' : 'msg-ai'}`}
            >
              <div className="msg-content">
                {msg.content.split('\n').map((line, j) => (
                  <p
                    key={j}
                    style={j > 0 ? { marginTop: 'var(--space-2)' } : undefined}
                  >
                    {line.startsWith('- ') ? (
                      <span>
                        {'• '}
                        {line.slice(2)}
                      </span>
                    ) : (
                      line
                    )}
                  </p>
                ))}
              </div>
              {msg.citations && msg.citations.length > 0 && (
                <div className="msg-evidence">
                  <div className="evidence-label">
                    EVIDENCE · {msg.citations.length} records
                  </div>
                  {msg.citations.map((c) => (
                    <div key={c.historyId} className="evidence-item">
                      <span className="mono dim">
                        {c.visitedAt.slice(0, 10)}
                      </span>
                      <span>{c.title ?? c.url}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          ))}
          {sending && (
            <div className="msg msg-ai">
              <div className="msg-content">
                <p className="dim">Thinking...</p>
              </div>
            </div>
          )}
        </div>
        <div className="assistant-input-area">
          <div className="assistant-input-wrapper">
            <input
              type="text"
              className="assistant-input"
              placeholder="Ask about your browsing history..."
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  void handleSend()
                }
              }}
              disabled={sending}
            />
            <button
              className="assistant-send"
              type="button"
              onClick={() => {
                void handleSend()
              }}
              disabled={sending}
            >
              <span>↵</span>
            </button>
          </div>
          <div className="assistant-hint dim">
            Powered by local LLM · Data never leaves your machine
          </div>
        </div>
      </div>
    </section>
  )
}
