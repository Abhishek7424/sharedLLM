import { useState, useEffect, useRef, useCallback } from 'react'
import { Link } from 'react-router-dom'
import {
  MessageSquare, Send, Plus, Square, Bot, User,
  AlertCircle, ChevronDown, Sparkles, Zap,
} from 'lucide-react'

// ─── Types ────────────────────────────────────────────────────────────────────

interface Message {
  id: string
  role: 'user' | 'assistant'
  content: string
}

interface Model {
  id: string
}

// ─── Constants ────────────────────────────────────────────────────────────────

const STORAGE_KEY   = 'sharedllm-chat-messages'
const MODEL_KEY     = 'sharedllm-chat-model'
const CHAT_TTL_MS   = 7 * 24 * 60 * 60 * 1000  // 7 days (VULN-20)

// Load messages, discarding any that are older than CHAT_TTL_MS
function loadMessages(): Message[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw)
    if (Array.isArray(parsed)) {
      // Legacy format (plain array): keep but treat as having no timestamp
      return parsed
    }
    if (parsed && typeof parsed === 'object' && Array.isArray(parsed.messages)) {
      const age = Date.now() - (parsed.savedAt ?? 0)
      if (age > CHAT_TTL_MS) return []
      return parsed.messages
    }
    return []
  } catch {
    return []
  }
}

const EXAMPLE_PROMPTS = [
  'Explain how distributed LLM inference works',
  'Write a Python script to benchmark GPU memory',
  'What are the pros and cons of running models locally?',
  'How do I quantize a model with llama.cpp?',
]

// ─── Chat page ────────────────────────────────────────────────────────────────

export function ChatPage() {
  const [messages, setMessages] = useState<Message[]>(() => loadMessages())
  const [input, setInput]               = useState('')
  const [isStreaming, setIsStreaming]   = useState(false)
  const [models, setModels]             = useState<Model[]>([])
  const [selectedModel, setSelectedModel] = useState(
    () => localStorage.getItem(MODEL_KEY) || ''
  )
  const [error, setError]               = useState<string | null>(null)
  const [inferenceUrl, setInferenceUrl] = useState<string | null>(null)

  const abortRef    = useRef<AbortController | null>(null)
  const bottomRef   = useRef<HTMLDivElement>(null)
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  // ── Fetch models on mount ──────────────────────────────────────────────────
  useEffect(() => {
    fetch('/v1/models')
      .then(r => r.json())
      .then(data => {
        const list: Model[] = data.data || []
        setModels(list)
        setSelectedModel(prev => {
          if (prev && list.find(m => m.id === prev)) return prev
          return list[0]?.id || ''
        })
      })
      .catch(() => {
        // Inference not running — show helpful empty state
      })

    // Also check backend config for the inference URL (for display)
    fetch('/api/backends/config')
      .then(r => r.json())
      .then(d => setInferenceUrl(d.inference_url || null))
      .catch(() => {})
  }, [])

  // ── Persist (with TTL timestamp) ──────────────────────────────────────────
  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({ messages, savedAt: Date.now() }))
  }, [messages])

  useEffect(() => {
    if (selectedModel) localStorage.setItem(MODEL_KEY, selectedModel)
  }, [selectedModel])

  // ── Auto-scroll ────────────────────────────────────────────────────────────
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  // ── Actions ────────────────────────────────────────────────────────────────
  const newChat = () => { setMessages([]); setError(null); setInput('') }

  const stopGeneration = () => {
    abortRef.current?.abort()
    setIsStreaming(false)
  }

  const send = useCallback(async (overrideText?: string) => {
    const text = (overrideText ?? input).trim()
    if (!text || isStreaming) return
    if (!selectedModel) {
      setError('No model loaded. Start inference from the Inference page first.')
      return
    }

    setError(null)
    setInput('')
    if (textareaRef.current) textareaRef.current.style.height = 'auto'

    const userMsg: Message      = { id: crypto.randomUUID(), role: 'user',      content: text }
    const assistantId           = crypto.randomUUID()
    const assistantMsg: Message = { id: assistantId,         role: 'assistant', content: '' }

    // Capture history before this turn for the API call
    const history = messages

    setMessages(prev => [...prev, userMsg, assistantMsg])
    setIsStreaming(true)

    const controller = new AbortController()
    abortRef.current = controller

    try {
      const resp = await fetch('/v1/chat/completions', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          model: selectedModel,
          messages: [...history, userMsg].map(m => ({ role: m.role, content: m.content })),
          stream: true,
        }),
        signal: controller.signal,
      })

      if (!resp.ok) {
        throw new Error(`Server responded ${resp.status}: ${await resp.text()}`)
      }

      const reader = resp.body!.getReader()
      const dec    = new TextDecoder()
      let buf      = ''

      while (true) {
        const { done, value } = await reader.read()
        if (done) break
        buf += dec.decode(value, { stream: true })
        const lines = buf.split('\n')
        buf = lines.pop() ?? ''

        for (const line of lines) {
          const s = line.trim()
          if (!s.startsWith('data: ')) continue
          const raw = s.slice(6)
          if (raw === '[DONE]') continue
          try {
            const delta = JSON.parse(raw).choices?.[0]?.delta?.content ?? ''
            if (delta) {
              setMessages(prev =>
                prev.map(m => m.id === assistantId ? { ...m, content: m.content + delta } : m)
              )
            }
          } catch { /* partial JSON — skip */ }
        }
      }
    } catch (err: any) {
      if (err.name === 'AbortError') return
      setError(err.message ?? 'Failed to reach the inference server.')
      setMessages(prev => prev.filter(m => m.id !== assistantId))
    } finally {
      setIsStreaming(false)
      abortRef.current = null
    }
  }, [input, isStreaming, selectedModel, messages])

  // ── Input helpers ──────────────────────────────────────────────────────────
  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); send() }
  }

  const handleInput = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setInput(e.target.value)
    e.target.style.height = 'auto'
    e.target.style.height = Math.min(e.target.scrollHeight, 160) + 'px'
  }

  const hasInference = models.length > 0

  // ── Render ─────────────────────────────────────────────────────────────────
  return (
    <div className="flex flex-col h-screen">

      {/* ── Header ── */}
      <div className="flex items-center justify-between px-5 py-3 border-b border-border bg-panel flex-shrink-0">
        <div className="flex items-center gap-2.5">
          <MessageSquare size={18} className="text-accent" />
          <span className="text-sm font-semibold text-gray-100">Chat</span>
          {hasInference && (
            <span className="text-xs text-muted/60 ml-1 hidden sm:inline">
              running locally
            </span>
          )}
        </div>

        <div className="flex items-center gap-2">
          {/* Model selector */}
          {hasInference && (
            <div className="relative">
              <select
                value={selectedModel}
                onChange={e => setSelectedModel(e.target.value)}
                className="appearance-none bg-surface border border-border rounded-lg pl-3 pr-7 py-1.5 text-xs text-gray-200 focus:outline-none focus:border-accent cursor-pointer max-w-[200px]"
              >
                {models.map(m => (
                  <option key={m.id} value={m.id}>{m.id}</option>
                ))}
              </select>
              <ChevronDown size={11} className="absolute right-2 top-1/2 -translate-y-1/2 text-muted pointer-events-none" />
            </div>
          )}

          {/* New chat button — only show when there are messages */}
          {messages.length > 0 && (
            <button
              onClick={newChat}
              className="flex items-center gap-1.5 px-3 py-1.5 text-xs text-muted hover:text-gray-300 border border-border rounded-lg hover:bg-white/5 transition-colors"
            >
              <Plus size={12} />
              New chat
            </button>
          )}
        </div>
      </div>

      {/* ── Messages ── */}
      <div className="flex-1 overflow-y-auto">
        {messages.length === 0 ? (
          <EmptyState
            hasInference={hasInference}
            inferenceUrl={inferenceUrl}
            onPrompt={p => {
              setInput(p)
              textareaRef.current?.focus()
            }}
          />
        ) : (
          <div className="max-w-3xl mx-auto px-4 py-6 space-y-6">
            {messages.map((msg, i) => (
              <MessageBubble
                key={msg.id}
                message={msg}
                streaming={isStreaming && i === messages.length - 1 && msg.role === 'assistant'}
              />
            ))}

            {error && (
              <div className="flex items-start gap-2 text-xs text-danger bg-danger/10 border border-danger/20 rounded-lg px-3 py-2.5">
                <AlertCircle size={13} className="flex-shrink-0 mt-0.5" />
                <span>{error}</span>
              </div>
            )}

            <div ref={bottomRef} />
          </div>
        )}
      </div>

      {/* ── Input bar ── */}
      <div className="flex-shrink-0 border-t border-border bg-panel px-4 py-3">
        <div className="max-w-3xl mx-auto">
          <div className="flex items-end gap-2">
            <textarea
              ref={textareaRef}
              value={input}
              onChange={handleInput}
              onKeyDown={handleKeyDown}
              placeholder={
                hasInference
                  ? 'Message… (Enter to send, Shift+Enter for newline)'
                  : 'Start inference from the Inference page first…'
              }
              disabled={!hasInference}
              rows={1}
              className="flex-1 bg-surface border border-border rounded-xl px-4 py-3 text-sm text-gray-100 placeholder-muted resize-none focus:outline-none focus:border-accent transition-colors disabled:opacity-40"
              style={{ minHeight: '48px', maxHeight: '160px' }}
            />

            {isStreaming ? (
              <button
                onClick={stopGeneration}
                className="flex-shrink-0 w-11 h-11 flex items-center justify-center rounded-xl bg-danger/10 hover:bg-danger/20 text-danger border border-danger/30 transition-colors"
                title="Stop generation"
              >
                <Square size={14} fill="currentColor" />
              </button>
            ) : (
              <button
                onClick={() => send()}
                disabled={!input.trim() || !hasInference}
                className="flex-shrink-0 w-11 h-11 flex items-center justify-center rounded-xl bg-accent hover:bg-accent-hover text-white transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
                title="Send (Enter)"
              >
                <Send size={15} />
              </button>
            )}
          </div>

          {selectedModel && (
            <p className="text-center text-xs text-muted/40 mt-2 truncate">{selectedModel}</p>
          )}
        </div>
      </div>
    </div>
  )
}

// ─── Empty state ──────────────────────────────────────────────────────────────

function EmptyState({
  hasInference,
  inferenceUrl,
  onPrompt,
}: {
  hasInference: boolean
  inferenceUrl: string | null
  onPrompt: (p: string) => void
}) {
  return (
    <div className="flex flex-col items-center justify-center h-full py-16 px-4 text-center">
      <div className="w-14 h-14 rounded-2xl bg-accent/10 border border-accent/20 flex items-center justify-center mb-5">
        <Sparkles size={26} className="text-accent" />
      </div>

      <h2 className="text-lg font-semibold text-gray-100 mb-1">What can I help with?</h2>

      {!hasInference ? (
        <div className="mt-3 max-w-sm">
          <p className="text-sm text-muted mb-5 leading-relaxed">
            No model is currently loaded. Start inference to begin chatting.
          </p>
          <Link
            to="/inference"
            className="inline-flex items-center gap-2 px-4 py-2.5 bg-accent hover:bg-accent-hover text-white text-sm font-medium rounded-lg transition-colors"
          >
            <Zap size={14} />
            Go to Inference
          </Link>
          {inferenceUrl && (
            <p className="text-xs text-muted mt-4 font-mono">{inferenceUrl}</p>
          )}
        </div>
      ) : (
        <>
          <p className="text-sm text-muted mb-8">Ask anything — running locally on your hardware.</p>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 max-w-lg w-full">
            {EXAMPLE_PROMPTS.map(p => (
              <button
                key={p}
                onClick={() => onPrompt(p)}
                className="text-left text-xs text-muted bg-panel border border-border rounded-xl px-4 py-3 hover:border-accent/40 hover:text-gray-300 transition-colors leading-relaxed"
              >
                {p}
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  )
}

// ─── Message bubble ───────────────────────────────────────────────────────────

function MessageBubble({ message, streaming }: { message: Message; streaming: boolean }) {
  const isUser = message.role === 'user'

  return (
    <div className={`flex gap-3 ${isUser ? 'flex-row-reverse' : 'flex-row'}`}>
      {/* Avatar */}
      <div
        className={`flex-shrink-0 w-7 h-7 rounded-lg flex items-center justify-center mt-0.5 ${
          isUser
            ? 'bg-accent/20 text-accent'
            : 'bg-panel border border-border text-muted'
        }`}
      >
        {isUser ? <User size={13} /> : <Bot size={13} />}
      </div>

      {/* Bubble */}
      <div className={`flex flex-col gap-1 ${isUser ? 'items-end' : 'items-start'} max-w-[85%]`}>
        <div
          className={`rounded-2xl px-4 py-3 text-sm leading-relaxed ${
            isUser
              ? 'bg-accent/15 text-gray-100 rounded-tr-sm'
              : 'bg-panel border border-border text-gray-200 rounded-tl-sm'
          }`}
        >
          {message.content ? (
            <MessageContent content={message.content} />
          ) : (
            streaming && (
              <span className="inline-flex items-center gap-1 py-0.5">
                <span className="w-1.5 h-1.5 rounded-full bg-accent/70 animate-bounce" style={{ animationDelay: '0ms' }} />
                <span className="w-1.5 h-1.5 rounded-full bg-accent/70 animate-bounce" style={{ animationDelay: '150ms' }} />
                <span className="w-1.5 h-1.5 rounded-full bg-accent/70 animate-bounce" style={{ animationDelay: '300ms' }} />
              </span>
            )
          )}
        </div>
      </div>
    </div>
  )
}

// ─── Message content renderer ─────────────────────────────────────────────────
// Handles: fenced code blocks, inline code, bold, newlines

function MessageContent({ content }: { content: string }) {
  // Split on fenced code blocks (``` ... ```)
  const segments = content.split(/(```[\s\S]*?```)/g)

  return (
    <>
      {segments.map((seg, i) => {
        if (seg.startsWith('```') && seg.endsWith('```')) {
          const inner     = seg.slice(3, -3)
          const newlineAt = inner.indexOf('\n')
          const lang      = newlineAt > -1 ? inner.slice(0, newlineAt).trim() : ''
          const code      = newlineAt > -1 ? inner.slice(newlineAt + 1) : inner
          return (
            <pre
              key={i}
              className="my-2 bg-surface rounded-lg px-4 py-3 text-xs font-mono text-gray-200 overflow-x-auto whitespace-pre"
            >
              {lang && (
                <div className="text-muted text-xs mb-2 pb-1 border-b border-border">{lang}</div>
              )}
              <code>{code}</code>
            </pre>
          )
        }

        // Inline text: handle `code` and **bold**, preserve newlines
        return (
          <span key={i} className="whitespace-pre-wrap">
            {seg.split(/(`[^`\n]+`|\*\*[^*]+\*\*)/g).map((part, j) => {
              if (part.startsWith('`') && part.endsWith('`')) {
                return (
                  <code
                    key={j}
                    className="bg-surface rounded px-1.5 py-0.5 text-xs font-mono text-accent"
                  >
                    {part.slice(1, -1)}
                  </code>
                )
              }
              if (part.startsWith('**') && part.endsWith('**')) {
                return (
                  <strong key={j} className="font-semibold text-gray-100">
                    {part.slice(2, -2)}
                  </strong>
                )
              }
              return part
            })}
          </span>
        )
      })}
    </>
  )
}
