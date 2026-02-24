import { useState, useEffect, useCallback } from 'react'
import { Download, Trash2, RefreshCw, Box, CheckCircle, Loader } from 'lucide-react'
import type { OllamaModel } from '../types'
import { api } from '../lib/api'

interface ModelsPageProps {
  ollamaRunning: boolean
  ollamaHost: string
}

function formatSize(bytes: number): string {
  const gb = bytes / (1024 ** 3)
  return gb >= 1 ? `${gb.toFixed(1)} GB` : `${(bytes / (1024 ** 2)).toFixed(0)} MB`
}

const POPULAR_MODELS = [
  'llama3.2:3b', 'llama3.2:1b', 'llama3.1:8b',
  'mistral:7b', 'phi3:mini', 'gemma2:2b',
  'qwen2.5:7b', 'deepseek-r1:7b', 'nomic-embed-text',
]

export function ModelsPage({ ollamaRunning, ollamaHost }: ModelsPageProps) {
  const [models, setModels] = useState<OllamaModel[]>([])
  const [loading, setLoading] = useState(true)
  const [pulling, setPulling] = useState<string | null>(null)
  const [pullProgress, setPullProgress] = useState<string>('')
  const [pullInput, setPullInput] = useState('')
  const [deleting, setDeleting] = useState<string | null>(null)
  const [pullError, setPullError] = useState<string | null>(null)

  const fetchModels = useCallback(async () => {
    if (!ollamaRunning) return
    setLoading(true)
    try {
      const data = await api.models()
      setModels(data.models ?? [])
    } catch {
      setModels([])
    } finally {
      setLoading(false)
    }
  }, [ollamaRunning])

  useEffect(() => { fetchModels() }, [fetchModels])

  const handlePull = async (name: string) => {
    setPulling(name)
    setPullProgress('')
    setPullError(null)
    try {
      const response = await api.pullModel(name)
      if (!response.ok) {
        setPullError(`Pull failed: HTTP ${response.status}`)
        return
      }
      // Stream NDJSON progress lines from the backend
      const reader = response.body?.getReader()
      const decoder = new TextDecoder()
      if (reader) {
        let buffer = ''
        while (true) {
          const { done, value } = await reader.read()
          if (done) break
          buffer += decoder.decode(value, { stream: true })
          const lines = buffer.split('\n')
          buffer = lines.pop() ?? ''
          for (const line of lines) {
            if (!line.trim()) continue
            try {
              const parsed = JSON.parse(line)
              if (parsed.status) {
                const pct = parsed.completed && parsed.total
                  ? ` (${Math.round((parsed.completed / parsed.total) * 100)}%)`
                  : ''
                setPullProgress(`${parsed.status}${pct}`)
              }
              if (parsed.error) {
                setPullError(parsed.error)
              }
            } catch {}
          }
        }
      }
      await fetchModels()
    } catch (e) {
      setPullError(e instanceof Error ? e.message : 'Pull failed')
    } finally {
      setPulling(null)
      setPullProgress('')
    }
  }

  const handleDelete = async (name: string) => {
    if (!confirm(`Delete model ${name}?`)) return
    setDeleting(name)
    try {
      await api.deleteModel(name)
      await fetchModels()
    } finally {
      setDeleting(null)
    }
  }

  if (!ollamaRunning) {
    return (
      <div className="p-6">
        <div className="card text-center py-16 space-y-3">
          <Box size={40} className="text-muted mx-auto" />
          <p className="text-gray-300 font-medium">Ollama is not running</p>
          <p className="text-sm text-muted">
            Install Ollama and start it, or enable auto-start in Settings.
          </p>
          <a href="https://ollama.ai" target="_blank" rel="noopener" className="btn-primary inline-flex mx-auto text-sm">
            Get Ollama
          </a>
        </div>
      </div>
    )
  }

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold text-gray-100">Models</h1>
          <p className="text-sm text-muted mt-0.5">Manage local LLMs via Ollama at {ollamaHost}</p>
        </div>
        <button onClick={fetchModels} className="btn-ghost text-sm" disabled={loading}>
          <RefreshCw size={15} className={loading ? 'animate-spin' : ''} />
        </button>
      </div>

      {/* Pull model */}
      <div className="card space-y-3">
        <p className="text-sm font-semibold text-gray-100">Pull a Model</p>
        <div className="flex gap-2">
          <input
            className="input text-sm flex-1"
            placeholder="e.g. llama3.2:3b"
            value={pullInput}
            onChange={e => setPullInput(e.target.value)}
            onKeyDown={e => e.key === 'Enter' && pullInput && handlePull(pullInput)}
          />
          <button
            onClick={() => pullInput && handlePull(pullInput)}
            disabled={!pullInput || !!pulling}
            className="btn-primary text-sm"
          >
            {pulling === pullInput ? <Loader size={15} className="animate-spin" /> : <Download size={15} />}
            {pulling === pullInput ? 'Pulling...' : 'Pull'}
          </button>
        </div>

        {/* Pull progress */}
        {pulling && pullProgress && (
          <p className="text-xs text-accent animate-pulse">{pullProgress}</p>
        )}
        {pullError && (
          <p className="text-xs text-danger">{pullError}</p>
        )}

        {/* Quick picks */}
        <div className="flex flex-wrap gap-1.5">
          {POPULAR_MODELS.map(m => (
            <button
              key={m}
              onClick={() => setPullInput(m)}
              className="text-xs px-2 py-1 rounded-md bg-surface border border-border text-muted hover:text-gray-300 hover:border-accent/40 transition-colors"
            >
              {m}
            </button>
          ))}
        </div>
      </div>

      {/* Model list */}
      {loading ? (
        <div className="card text-center text-muted py-8 text-sm">Loading models...</div>
      ) : models.length === 0 ? (
        <div className="card text-center py-12 space-y-2">
          <Box size={32} className="text-muted mx-auto" />
          <p className="text-muted text-sm">No local models. Pull one above.</p>
        </div>
      ) : (
        <div className="space-y-2">
          {models.map(model => (
            <div key={model.name} className="card flex items-center gap-4">
              <div className="w-9 h-9 rounded-lg bg-accent/15 text-accent flex items-center justify-center flex-shrink-0">
                <Box size={18} />
              </div>
              <div className="flex-1 min-w-0">
                <p className="font-semibold text-sm text-gray-100 truncate">{model.name}</p>
                <div className="flex gap-3 text-xs text-muted mt-0.5">
                  <span>{formatSize(model.size)}</span>
                  <span className="font-mono truncate max-w-[120px]">{model.digest.slice(0, 12)}</span>
                </div>
              </div>
              <div className="flex items-center gap-2">
                <span className="badge-approved text-xs">
                  <CheckCircle size={11} /> local
                </span>
                <button
                  onClick={() => handleDelete(model.name)}
                  disabled={deleting === model.name}
                  className="btn-danger text-xs py-1.5 px-2.5"
                >
                  {deleting === model.name
                    ? <Loader size={13} className="animate-spin" />
                    : <Trash2 size={13} />}
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
