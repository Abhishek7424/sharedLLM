import { useState, useEffect, useRef } from 'react'
import { MessageSquare, RefreshCw, ExternalLink, Loader2 } from 'lucide-react'

const OPENWEBUI_URL = 'http://localhost:3001'
const STATUS_API = '/api/openwebui/status'
const POLL_INTERVAL_MS = 3000

type Status = 'starting' | 'online'

export function ChatPage() {
  const [status, setStatus] = useState<Status>('starting')
  const [iframeKey, setIframeKey] = useState(0)
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const checkStatus = async () => {
    try {
      const res = await fetch(STATUS_API, { cache: 'no-store' })
      const data = await res.json()
      if (data.running) {
        setStatus('online')
        // Stop polling once we're online
        if (intervalRef.current) {
          clearInterval(intervalRef.current)
          intervalRef.current = null
        }
      } else {
        setStatus('starting')
      }
    } catch {
      // Backend itself is unreachable — keep showing spinner
      setStatus('starting')
    }
  }

  useEffect(() => {
    checkStatus()
    intervalRef.current = setInterval(checkStatus, POLL_INTERVAL_MS)
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current)
    }
  }, [])

  const reload = () => {
    setStatus('starting')
    setIframeKey(k => k + 1)
    // Resume polling
    if (!intervalRef.current) {
      intervalRef.current = setInterval(checkStatus, POLL_INTERVAL_MS)
    }
    checkStatus()
  }

  return (
    <div className="flex flex-col" style={{ height: '100vh' }}>
      {/* Header bar */}
      <div className="flex items-center justify-between px-5 py-3 border-b border-border bg-panel flex-shrink-0">
        <div className="flex items-center gap-2.5">
          <MessageSquare size={18} className="text-accent" />
          <span className="text-sm font-semibold text-gray-100">Chat</span>
          <span className="text-xs text-muted ml-1">powered by Open WebUI</span>
        </div>
        <div className="flex items-center gap-3">
          {/* Status pill */}
          <div className="flex items-center gap-1.5 text-xs">
            {status === 'starting' && (
              <>
                <Loader2 size={12} className="animate-spin text-muted" />
                <span className="text-muted">Starting up…</span>
              </>
            )}
            {status === 'online' && (
              <>
                <span className="w-1.5 h-1.5 rounded-full bg-success" />
                <span className="text-success">Open WebUI running</span>
              </>
            )}
          </div>

          <button
            onClick={reload}
            className="p-1.5 rounded hover:bg-white/5 text-muted hover:text-gray-300 transition-colors"
            title="Reload"
          >
            <RefreshCw size={14} />
          </button>
          <a
            href={OPENWEBUI_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="p-1.5 rounded hover:bg-white/5 text-muted hover:text-gray-300 transition-colors"
            title="Open in new tab"
          >
            <ExternalLink size={14} />
          </a>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 relative overflow-hidden">
        {/* iframe is always mounted so it loads the moment status flips */}
        <iframe
          key={iframeKey}
          src={OPENWEBUI_URL}
          title="Open WebUI Chat"
          className="w-full h-full border-0"
          allow="microphone; camera; clipboard-read; clipboard-write"
          style={{ display: status === 'online' ? 'block' : 'none' }}
        />

        {status === 'starting' && (
          <div className="absolute inset-0 flex items-center justify-center bg-surface">
            <div className="flex flex-col items-center gap-4 text-center">
              <Loader2 size={36} className="animate-spin text-accent" />
              <div>
                <p className="text-sm font-medium text-gray-200 mb-1">Chat is starting up…</p>
                <p className="text-xs text-muted">
                  Open WebUI is warming up. This takes about 30 seconds on first launch.
                </p>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
