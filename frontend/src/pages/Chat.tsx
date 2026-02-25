import { useState, useEffect, useRef } from 'react'
import { MessageSquare, RefreshCw, ExternalLink, AlertCircle, Loader2 } from 'lucide-react'

const OPENWEBUI_URL = 'http://localhost:3001'
const POLL_INTERVAL_MS = 3000

type Status = 'checking' | 'online' | 'offline'

export function ChatPage() {
  const [status, setStatus] = useState<Status>('checking')
  const [iframeKey, setIframeKey] = useState(0)
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const checkStatus = async () => {
    try {
      // Use a no-cors fetch — we just care if the server responds
      await fetch(OPENWEBUI_URL, { mode: 'no-cors', cache: 'no-store' })
      setStatus('online')
    } catch {
      setStatus('offline')
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
    setStatus('checking')
    setIframeKey(k => k + 1)
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
            {status === 'checking' && (
              <>
                <Loader2 size={12} className="animate-spin text-muted" />
                <span className="text-muted">Connecting…</span>
              </>
            )}
            {status === 'online' && (
              <>
                <span className="w-1.5 h-1.5 rounded-full bg-success" />
                <span className="text-success">Open WebUI running</span>
              </>
            )}
            {status === 'offline' && (
              <>
                <span className="w-1.5 h-1.5 rounded-full bg-danger" />
                <span className="text-danger">Open WebUI offline</span>
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
        {status === 'offline' ? (
          <OfflineState onRetry={reload} />
        ) : (
          <iframe
            key={iframeKey}
            src={OPENWEBUI_URL}
            title="Open WebUI Chat"
            className="w-full h-full border-0"
            allow="microphone; camera; clipboard-read; clipboard-write"
            style={{ display: status === 'checking' ? 'none' : 'block' }}
          />
        )}

        {status === 'checking' && (
          <div className="absolute inset-0 flex items-center justify-center bg-surface">
            <div className="flex flex-col items-center gap-3 text-center">
              <Loader2 size={32} className="animate-spin text-accent" />
              <p className="text-sm text-muted">Connecting to Open WebUI…</p>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

function OfflineState({ onRetry }: { onRetry: () => void }) {
  return (
    <div className="flex items-center justify-center h-full bg-surface">
      <div className="max-w-md text-center px-6">
        <div className="w-14 h-14 rounded-2xl bg-danger/10 flex items-center justify-center mx-auto mb-4">
          <AlertCircle size={28} className="text-danger" />
        </div>
        <h2 className="text-lg font-semibold text-gray-100 mb-2">Open WebUI is not running</h2>
        <p className="text-sm text-muted mb-6 leading-relaxed">
          Start Open WebUI by running the script below in a terminal. It will launch on{' '}
          <code className="text-accent bg-accent/10 px-1 py-0.5 rounded text-xs">
            localhost:3001
          </code>{' '}
          and automatically connect to your SharedLLM inference backend.
        </p>

        {/* Start command */}
        <div className="bg-panel border border-border rounded-xl p-4 mb-6 text-left">
          <p className="text-xs text-muted mb-2 font-medium uppercase tracking-wide">
            Terminal command
          </p>
          <code className="text-xs text-accent font-mono break-all">
            ./start-openwebui.sh
          </code>
        </div>

        <div className="flex flex-col gap-2">
          <button
            onClick={onRetry}
            className="w-full py-2.5 px-4 bg-accent text-white text-sm font-medium rounded-lg hover:bg-accent/90 transition-colors flex items-center justify-center gap-2"
          >
            <RefreshCw size={14} />
            Check again
          </button>
          <a
            href={OPENWEBUI_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="w-full py-2.5 px-4 border border-border text-sm text-muted rounded-lg hover:text-gray-300 hover:border-gray-600 transition-colors flex items-center justify-center gap-2"
          >
            <ExternalLink size={14} />
            Open localhost:3001 in browser
          </a>
        </div>
      </div>
    </div>
  )
}
