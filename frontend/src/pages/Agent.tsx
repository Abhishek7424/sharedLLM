import { useState, useEffect } from 'react'
import { Link } from 'react-router-dom'
import { Copy, Check, Terminal, Monitor, Server } from 'lucide-react'
import { clsx } from 'clsx'
import { api } from '../lib/api'
import type { AgentInfo } from '../types'

// ─── Copy button ──────────────────────────────────────────────────────────────

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false)

  async function copy() {
    try {
      await navigator.clipboard.writeText(text)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch {}
  }

  return (
    <button
      onClick={copy}
      className="p-1 rounded hover:bg-white/10 text-muted hover:text-gray-300 transition-colors"
      title="Copy to clipboard"
    >
      {copied ? <Check size={14} className="text-success" /> : <Copy size={14} />}
    </button>
  )
}

// ─── Code block ───────────────────────────────────────────────────────────────

function CodeBlock({ code, label }: { code: string; label?: string }) {
  return (
    <div className="relative">
      {label && <p className="text-xs text-muted mb-1">{label}</p>}
      <div className="bg-surface border border-border rounded-lg px-4 py-3 pr-10 text-sm font-mono text-gray-200 break-all">
        {code}
        <div className="absolute top-2 right-2">
          <CopyButton text={code} />
        </div>
      </div>
    </div>
  )
}

// ─── OS Tab ───────────────────────────────────────────────────────────────────

type OsTab = 'linux' | 'macos' | 'windows'

const osTabs: { id: OsTab; label: string; icon: React.ElementType }[] = [
  { id: 'linux', label: 'Linux', icon: Terminal },
  { id: 'macos', label: 'macOS', icon: Monitor },
  { id: 'windows', label: 'Windows', icon: Server },
]

// ─── Main Page ────────────────────────────────────────────────────────────────

export function AgentPage() {
  const [info, setInfo] = useState<AgentInfo | null>(null)
  const [activeOs, setActiveOs] = useState<OsTab>('linux')
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    api.agentInfo()
      .then(setInfo)
      .catch(e => setError(e.message))
  }, [])

  return (
    <div className="p-6 space-y-6">
      <div>
        <h1 className="text-xl font-bold text-gray-100">Agent Install</h1>
        <p className="text-sm text-muted mt-0.5">
          Run this on each machine you want to add to the inference cluster
        </p>
      </div>

      {error && (
        <div className="card bg-danger/10 border-danger/20 text-danger text-sm">
          Failed to load agent info: {error}
        </div>
      )}

      {/* How it works */}
      <div className="card">
        <h2 className="text-sm font-semibold text-gray-300 mb-3">How it works</h2>
        <ol className="space-y-2 text-sm text-muted">
          <li className="flex gap-3">
            <span className="w-5 h-5 rounded-full bg-accent/20 text-accent text-xs flex items-center justify-center flex-shrink-0 mt-0.5">1</span>
            <span>Run the one-liner below on each device you want to use for inference.</span>
          </li>
          <li className="flex gap-3">
            <span className="w-5 h-5 rounded-full bg-accent/20 text-accent text-xs flex items-center justify-center flex-shrink-0 mt-0.5">2</span>
            <span>The script installs <code className="font-mono text-xs bg-surface px-1 py-0.5 rounded">llama-rpc-server</code> and starts it on port{' '}
              <code className="font-mono text-xs bg-surface px-1 py-0.5 rounded">{info?.rpc_port ?? 8181}</code>.
            </span>
          </li>
          <li className="flex gap-3">
            <span className="w-5 h-5 rounded-full bg-accent/20 text-accent text-xs flex items-center justify-center flex-shrink-0 mt-0.5">3</span>
            <span>The script automatically registers this device with the host. Approve it in the{' '}
              <Link to="/devices" className="text-accent hover:underline">Devices</Link>{' '}
              tab.
            </span>
          </li>
          <li className="flex gap-3">
            <span className="w-5 h-5 rounded-full bg-accent/20 text-accent text-xs flex items-center justify-center flex-shrink-0 mt-0.5">4</span>
            <span>Go to{' '}
              <Link to="/inference" className="text-accent hover:underline">Inference</Link>{' '}
              and select the device to distribute model layers across the cluster.
            </span>
          </li>
        </ol>
      </div>

      {/* Install commands */}
      <div className="card">
        <h2 className="text-sm font-semibold text-gray-300 mb-3">Install Command</h2>

        {/* OS tabs */}
        <div className="flex gap-1 mb-4 bg-surface rounded-lg p-1 w-fit">
          {osTabs.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              onClick={() => setActiveOs(id)}
              className={clsx(
                'flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-colors',
                activeOs === id
                  ? 'bg-accent text-white'
                  : 'text-muted hover:text-gray-300'
              )}
            >
              <Icon size={12} />
              {label}
            </button>
          ))}
        </div>

        {info && (
          <CodeBlock
            code={info.install_commands[activeOs]}
            label={`Run on the target ${activeOs} machine:`}
          />
        )}

        {!info && !error && (
          <div className="h-12 bg-surface rounded-lg animate-pulse" />
        )}

        {activeOs === 'windows' && (
          <p className="text-xs text-muted mt-2">
            Run in PowerShell as Administrator.
          </p>
        )}
      </div>

      {/* Manual install */}
      <div className="card">
        <h2 className="text-sm font-semibold text-gray-300 mb-3">Manual Install</h2>
        <p className="text-sm text-muted mb-3">
          If you prefer to install manually or already have llama.cpp installed:
        </p>
        <div className="space-y-3">
          <CodeBlock
            code="# Install llama.cpp (macOS with Homebrew)"
            label="Option A — Homebrew:"
          />
          <CodeBlock code="brew install llama.cpp" />

          <CodeBlock
            code={`# Then start the RPC server\nllama-rpc-server --host 0.0.0.0 --port ${info?.rpc_port ?? 8181}`}
            label="Option B — After installing, start the server:"
          />

          <div className="mt-2">
            <p className="text-xs text-muted">
              Download pre-built binaries:{' '}
              <a
                href="https://github.com/ggml-org/llama.cpp/releases"
                target="_blank"
                rel="noopener noreferrer"
                className="text-accent hover:underline"
              >
                github.com/ggml-org/llama.cpp/releases
              </a>
            </p>
          </div>
        </div>
      </div>

      {/* Connection info */}
      {info && (
        <div className="card">
          <h2 className="text-sm font-semibold text-gray-300 mb-3">Connection Details</h2>
          <div className="grid grid-cols-2 gap-3 text-sm">
            <div>
              <p className="text-xs text-muted mb-1">Host IP (this machine)</p>
              <code className="font-mono text-gray-200">{info.host_ip}</code>
            </div>
            <div>
              <p className="text-xs text-muted mb-1">Dashboard Port</p>
              <code className="font-mono text-gray-200">{info.dashboard_port}</code>
            </div>
            <div>
              <p className="text-xs text-muted mb-1">RPC Port (agents)</p>
              <code className="font-mono text-gray-200">{info.rpc_port}</code>
            </div>
            <div>
              <p className="text-xs text-muted mb-1">llama-rpc-server status</p>
              <span className={clsx(
                'text-xs font-medium',
                info.rpc_server_bin_available ? 'text-success' : 'text-warning'
              )}>
                {info.rpc_server_bin_available ? 'Found in PATH' : 'Not found'}
              </span>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
