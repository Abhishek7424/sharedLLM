import { useState, useEffect, useRef, useCallback } from 'react'
import { Play, Square, Cpu, Wifi, WifiOff, Send, Loader2, RefreshCw } from 'lucide-react'
import { clsx } from 'clsx'
import { api } from '../lib/api'
import type { ClusterStatus, ChatMessage, InferenceSessionInfo } from '../types'

// ─── Helpers ──────────────────────────────────────────────────────────────────

function fmt(mb: number) {
  return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${mb} MB`
}

function StatusDot({ ok }: { ok: boolean }) {
  return (
    <span
      className={clsx(
        'inline-block w-2 h-2 rounded-full flex-shrink-0',
        ok ? 'bg-success' : 'bg-danger'
      )}
    />
  )
}

// ─── Chat ─────────────────────────────────────────────────────────────────────

function ChatPanel({ inferenceRunning }: { inferenceRunning: boolean }) {
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [input, setInput] = useState('')
  const [loading, setLoading] = useState(false)
  const [tokens, setTokens] = useState<number | null>(null)
  const endRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  async function send() {
    if (!input.trim() || loading || !inferenceRunning) return

    const userMsg: ChatMessage = { role: 'user', content: input.trim() }
    setMessages(prev => [...prev, userMsg])
    setInput('')
    setLoading(true)

    const history: ChatMessage[] = [...messages, userMsg]
    const t0 = Date.now()

    try {
      const resp = await api.chatCompletions(history)
      if (!resp.ok) {
        const err = await resp.json().catch(() => ({ error: 'Unknown error' }))
        setMessages(prev => [
          ...prev,
          { role: 'assistant', content: `Error: ${err.error ?? resp.status}` },
        ])
        return
      }

      const data = await resp.json()
      const content =
        data?.choices?.[0]?.message?.content ??
        data?.choices?.[0]?.text ??
        '(empty response)'

      const elapsed = (Date.now() - t0) / 1000
      const tokenCount = data?.usage?.completion_tokens
      if (tokenCount) setTokens(Math.round(tokenCount / elapsed))

      setMessages(prev => [...prev, { role: 'assistant', content }])
    } catch (e: unknown) {
      setMessages(prev => [
        ...prev,
        { role: 'assistant', content: `Error: ${e instanceof Error ? e.message : String(e)}` },
      ])
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="card flex flex-col h-[500px]">
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-sm font-semibold text-gray-200">Chat</h3>
        {tokens !== null && (
          <span className="text-xs text-muted">{tokens} tok/s</span>
        )}
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto space-y-3 mb-3 pr-1">
        {messages.length === 0 && (
          <p className="text-xs text-muted text-center mt-8">
            {inferenceRunning
              ? 'Start chatting with the model...'
              : 'Start inference first to enable chat.'}
          </p>
        )}
        {messages.map((m, i) => (
          <div
            key={i}
            className={clsx(
              'rounded-lg px-3 py-2 text-sm max-w-[85%] whitespace-pre-wrap',
              m.role === 'user'
                ? 'ml-auto bg-accent text-white'
                : 'bg-surface text-gray-200 border border-border'
            )}
          >
            {m.content}
          </div>
        ))}
        {loading && (
          <div className="bg-surface text-gray-400 border border-border rounded-lg px-3 py-2 text-sm flex items-center gap-2">
            <Loader2 size={14} className="animate-spin" />
            Generating...
          </div>
        )}
        <div ref={endRef} />
      </div>

      {/* Input */}
      <div className="flex gap-2">
        <input
          value={input}
          onChange={e => setInput(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && !e.shiftKey && send()}
          placeholder={inferenceRunning ? 'Type a message...' : 'Start inference to chat'}
          disabled={!inferenceRunning || loading}
          className="flex-1 bg-surface border border-border rounded-lg px-3 py-2 text-sm text-gray-200 placeholder-muted disabled:opacity-50 focus:outline-none focus:border-accent"
        />
        <button
          onClick={send}
          disabled={!inferenceRunning || loading || !input.trim()}
          className="btn-primary p-2 disabled:opacity-40"
        >
          <Send size={16} />
        </button>
      </div>
    </div>
  )
}

// ─── Main Page ────────────────────────────────────────────────────────────────

export function InferencePage() {
  const [clusterStatus, setClusterStatus] = useState<ClusterStatus | null>(null)
  const [selectedDeviceIds, setSelectedDeviceIds] = useState<string[]>([])
  const [modelPath, setModelPath] = useState('')
  const [loading, setLoading] = useState(false)
  const [actionError, setActionError] = useState<string | null>(null)
  const [refreshing, setRefreshing] = useState(false)

  const refresh = useCallback(async () => {
    setRefreshing(true)
    try {
      const data = await api.clusterStatus()
      setClusterStatus(data)
    } catch (e: unknown) {
      console.error('Failed to fetch cluster status', e)
    } finally {
      setRefreshing(false)
    }
  }, [])

  useEffect(() => {
    refresh()
    const id = setInterval(refresh, 5000)
    return () => clearInterval(id)
  }, [refresh])

  const session: InferenceSessionInfo | undefined = clusterStatus?.current_session
  const inferenceRunning = clusterStatus?.llama_cpp.inference_running ?? false
  const rpcBinAvailable = clusterStatus?.llama_cpp.rpc_server_bin ?? false
  const inferenceBinAvailable = clusterStatus?.llama_cpp.inference_server_bin ?? false

  async function handleStart() {
    if (!modelPath.trim()) {
      setActionError('Please enter a model path (.gguf file).')
      return
    }
    setLoading(true)
    setActionError(null)
    try {
      await api.startInference(modelPath.trim(), selectedDeviceIds)
      await refresh()
    } catch (e: unknown) {
      setActionError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }

  async function handleStop() {
    setLoading(true)
    setActionError(null)
    try {
      await api.stopInference()
      await refresh()
    } catch (e: unknown) {
      setActionError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }

  async function handleRpcToggle() {
    setLoading(true)
    try {
      const running = clusterStatus?.llama_cpp.rpc_server_running
      if (running) {
        await api.stopRpcServer()
      } else {
        await api.startRpcServer()
      }
      await refresh()
    } catch (e: unknown) {
      setActionError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }

  function toggleDevice(id: string) {
    setSelectedDeviceIds(prev =>
      prev.includes(id) ? prev.filter(d => d !== id) : [...prev, id]
    )
  }

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold text-gray-100">Distributed Inference</h1>
          <p className="text-sm text-muted mt-0.5">
            Run LLM inference across multiple devices using llama.cpp RPC
          </p>
        </div>
        <button
          onClick={refresh}
          disabled={refreshing}
          className="btn-ghost flex items-center gap-2 text-sm"
        >
          <RefreshCw size={14} className={refreshing ? 'animate-spin' : ''} />
          Refresh
        </button>
      </div>

      {/* Binary status */}
      <div className="card">
        <h2 className="text-sm font-semibold text-gray-300 mb-3">llama.cpp Binaries</h2>
        <div className="grid grid-cols-2 gap-3">
          <div className="flex items-center gap-2 text-sm">
            <StatusDot ok={rpcBinAvailable} />
            <span className={rpcBinAvailable ? 'text-gray-300' : 'text-muted'}>
              llama-rpc-server {rpcBinAvailable ? '(found in PATH)' : '(not found)'}
            </span>
          </div>
          <div className="flex items-center gap-2 text-sm">
            <StatusDot ok={inferenceBinAvailable} />
            <span className={inferenceBinAvailable ? 'text-gray-300' : 'text-muted'}>
              llama-server {inferenceBinAvailable ? '(found in PATH)' : '(not found)'}
            </span>
          </div>
        </div>
        {(!rpcBinAvailable || !inferenceBinAvailable) && (
          <p className="text-xs text-warning mt-2">
            Install llama.cpp and add the binaries to your PATH, or place them in{' '}
            <code className="font-mono">~/.sharedmem/bin/</code>.{' '}
            <a
              href="https://github.com/ggerganov/llama.cpp/releases"
              target="_blank"
              rel="noopener noreferrer"
              className="text-accent hover:underline"
            >
              Download from GitHub
            </a>
          </p>
        )}
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-2 gap-6">
        {/* Left: controls */}
        <div className="space-y-4">
          {/* Local RPC server */}
          <div className="card">
            <div className="flex items-center justify-between mb-3">
              <div>
                <h2 className="text-sm font-semibold text-gray-300">Local RPC Server</h2>
                <p className="text-xs text-muted mt-0.5">
                  Exposes this machine's GPU to the cluster (port{' '}
                  {clusterStatus?.llama_cpp.rpc_port ?? 8181})
                </p>
              </div>
              <div className="flex items-center gap-2">
                <StatusDot ok={clusterStatus?.llama_cpp.rpc_server_running ?? false} />
                <span className="text-xs text-muted">
                  {clusterStatus?.llama_cpp.rpc_server_running ? 'Running' : 'Stopped'}
                </span>
              </div>
            </div>
            <button
              onClick={handleRpcToggle}
              disabled={loading || !rpcBinAvailable}
              className={clsx(
                'w-full py-1.5 text-sm rounded-lg transition-colors disabled:opacity-40',
                clusterStatus?.llama_cpp.rpc_server_running
                  ? 'bg-danger/15 text-danger hover:bg-danger/25'
                  : 'bg-success/15 text-success hover:bg-success/25'
              )}
            >
              {clusterStatus?.llama_cpp.rpc_server_running ? 'Stop RPC Server' : 'Start RPC Server'}
            </button>
          </div>

          {/* Model path */}
          <div className="card">
            <h2 className="text-sm font-semibold text-gray-300 mb-3">Model</h2>
            <input
              value={modelPath}
              onChange={e => setModelPath(e.target.value)}
              placeholder="/path/to/model.gguf"
              disabled={inferenceRunning}
              className="w-full bg-surface border border-border rounded-lg px-3 py-2 text-sm text-gray-200 placeholder-muted disabled:opacity-50 focus:outline-none focus:border-accent font-mono"
            />
            <p className="text-xs text-muted mt-1.5">
              Full path to a .gguf model file on this machine.
            </p>
          </div>

          {/* Device selection */}
          <div className="card">
            <h2 className="text-sm font-semibold text-gray-300 mb-3">
              Network Devices{' '}
              <span className="text-muted font-normal">
                ({clusterStatus?.devices.length ?? 0} approved)
              </span>
            </h2>
            {clusterStatus?.devices.length === 0 ? (
              <p className="text-xs text-muted">
                No approved devices. Go to{' '}
                <a href="/devices" className="text-accent hover:underline">
                  Devices
                </a>{' '}
                to approve network machines, then have them run the agent (see{' '}
                <a href="/agent" className="text-accent hover:underline">
                  Agent
                </a>
                ).
              </p>
            ) : (
              <div className="space-y-2">
                {clusterStatus?.devices.map(device => {
                  const ready = device.rpc_status === 'ready'
                  return (
                    <label
                      key={device.id}
                      className={clsx(
                        'flex items-center gap-3 p-2 rounded-lg border cursor-pointer transition-colors',
                        selectedDeviceIds.includes(device.id)
                          ? 'border-accent/40 bg-accent/5'
                          : 'border-border hover:border-border/80'
                      )}
                    >
                      <input
                        type="checkbox"
                        checked={selectedDeviceIds.includes(device.id)}
                        onChange={() => toggleDevice(device.id)}
                        disabled={inferenceRunning}
                        className="accent-accent"
                      />
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2">
                          {ready ? (
                            <Wifi size={12} className="text-success flex-shrink-0" />
                          ) : (
                            <WifiOff size={12} className="text-muted flex-shrink-0" />
                          )}
                          <span className="text-sm text-gray-200 truncate">{device.name}</span>
                          <span className="text-xs text-muted font-mono">{device.ip}</span>
                        </div>
                        {device.memory_total_mb > 0 && (
                          <p className="text-xs text-muted mt-0.5 ml-5">
                            {fmt(device.memory_free_mb)} free / {fmt(device.memory_total_mb)} total
                          </p>
                        )}
                      </div>
                      <span
                        className={clsx(
                          'text-xs px-1.5 py-0.5 rounded',
                          ready
                            ? 'bg-success/15 text-success'
                            : 'bg-surface text-muted'
                        )}
                      >
                        {device.rpc_status}
                      </span>
                    </label>
                  )
                })}
              </div>
            )}
          </div>

          {/* Start/Stop */}
          {actionError && (
            <div className="text-xs text-danger bg-danger/10 border border-danger/20 rounded-lg px-3 py-2">
              {actionError}
            </div>
          )}

          {session && inferenceRunning && (
            <div className="card bg-success/5 border-success/20">
              <div className="flex items-center gap-2 mb-1">
                <Cpu size={14} className="text-success" />
                <span className="text-sm font-semibold text-success">Inference Running</span>
              </div>
              <p className="text-xs text-muted truncate">{session.model_path}</p>
              {session.rpc_devices.length > 0 && (
                <p className="text-xs text-muted mt-0.5">
                  Distributed across: {session.rpc_devices.join(', ')}
                </p>
              )}
            </div>
          )}

          <div className="flex gap-3">
            {!inferenceRunning ? (
              <button
                onClick={handleStart}
                disabled={loading || !inferenceBinAvailable}
                className="flex-1 btn-primary flex items-center justify-center gap-2 disabled:opacity-40"
              >
                {loading ? <Loader2 size={15} className="animate-spin" /> : <Play size={15} />}
                Start Inference
              </button>
            ) : (
              <button
                onClick={handleStop}
                disabled={loading}
                className="flex-1 bg-danger/15 text-danger hover:bg-danger/25 rounded-lg py-2 text-sm font-medium transition-colors flex items-center justify-center gap-2 disabled:opacity-40"
              >
                {loading ? <Loader2 size={15} className="animate-spin" /> : <Square size={15} />}
                Stop Inference
              </button>
            )}
          </div>
        </div>

        {/* Right: chat */}
        <div>
          <ChatPanel inferenceRunning={inferenceRunning} />

          {/* Inference server info */}
          {inferenceRunning && (
            <div className="mt-3 text-xs text-muted">
              Inference server running on port{' '}
              <code className="font-mono">{clusterStatus?.llama_cpp.inference_port ?? 8282}</code>.
              Also accessible at{' '}
              <code className="font-mono">
                http://localhost:{clusterStatus?.llama_cpp.inference_port ?? 8282}/v1/chat/completions
              </code>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
