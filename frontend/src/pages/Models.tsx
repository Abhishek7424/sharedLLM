import { useState, useEffect, useCallback } from 'react'
import { RefreshCw, Box, Terminal } from 'lucide-react'
import { Link } from 'react-router-dom'
import { api } from '../lib/api'

interface ModelEntry {
  id: string
  object?: string
  owned_by?: string
}

export function ModelsPage() {
  const [models, setModels] = useState<ModelEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const fetchModels = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const resp = await fetch(`${api.base}/v1/models`)
      if (!resp.ok) {
        setError(`HTTP ${resp.status}`)
        setModels([])
        return
      }
      const data = await resp.json()
      setModels(data.data ?? [])
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to fetch models')
      setModels([])
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => { fetchModels() }, [fetchModels])

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold text-gray-100">Models</h1>
          <p className="text-sm text-muted mt-0.5">
            Models available from the active inference backend
          </p>
        </div>
        <button onClick={fetchModels} disabled={loading} className="btn-ghost text-sm flex items-center gap-2">
          <RefreshCw size={15} className={loading ? 'animate-spin' : ''} />
          Refresh
        </button>
      </div>

      {loading ? (
        <div className="card text-center text-muted py-8 text-sm">Loading models...</div>
      ) : error ? (
        <div className="card text-center py-12 space-y-3">
          <Box size={36} className="text-muted mx-auto" />
          <p className="text-gray-300 font-medium">No active inference backend</p>
          <p className="text-sm text-muted">
            Start llama.cpp inference or activate an external backend on the{' '}
            <Link to="/inference" className="text-accent hover:underline">Inference</Link> page.
          </p>
        </div>
      ) : models.length === 0 ? (
        <div className="card text-center py-12 space-y-3">
          <Box size={36} className="text-muted mx-auto" />
          <p className="text-muted text-sm">No models reported by the active backend.</p>
          <p className="text-xs text-muted">
            Load a .gguf file on the{' '}
            <Link to="/inference" className="text-accent hover:underline">Inference</Link> page to get started.
          </p>
        </div>
      ) : (
        <div className="space-y-2">
          {models.map(model => (
            <div key={model.id} className="card flex items-center gap-4">
              <div className="w-9 h-9 rounded-lg bg-accent/15 text-accent flex items-center justify-center flex-shrink-0">
                <Terminal size={18} />
              </div>
              <div className="flex-1 min-w-0">
                <p className="font-semibold text-sm text-gray-100 truncate">{model.id}</p>
                {model.owned_by && (
                  <p className="text-xs text-muted mt-0.5">{model.owned_by}</p>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
