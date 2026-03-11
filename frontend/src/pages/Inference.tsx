import { useState, useEffect, useRef, useCallback } from 'react'
import { Link } from 'react-router-dom'
import { Play, Square, Cpu, Wifi, WifiOff, Send, Loader2, RefreshCw, Download, Check, ChevronDown, AlertTriangle, FolderSearch } from 'lucide-react'
import { clsx } from 'clsx'
import { api } from '../lib/api'
import type { BackendConfig, BackendType, ClusterStatus, ChatMessage, InferenceSessionInfo, ModelCheckResult, FitStatus } from '../types'

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

// ─── Backend tab labels ───────────────────────────────────────────────────────

// ─── ModelGuardrails ──────────────────────────────────────────────────────────

const FIT_META: Record<FitStatus, { label: string; badgeCls: string; barCls: string }> = {
  fits_locally:      { label: 'Fits in Local Memory',    badgeCls: 'bg-success/15 text-success',  barCls: 'bg-success/50' },
  fits_distributed:  { label: 'Fits Across Cluster',     badgeCls: 'bg-accent/15 text-accent',    barCls: 'bg-accent/50' },
  partial_gpu:       { label: 'Partial GPU',              badgeCls: 'bg-warning/15 text-warning',  barCls: 'bg-warning/50' },
  too_large:         { label: 'Too Large for Cluster',    badgeCls: 'bg-danger/15 text-danger',    barCls: 'bg-danger/50' },
}

interface ModelGuardrailsProps {
  modelPath: string
  selectedDeviceIds: string[]
  disabled: boolean
  onSettingsChange: (s: { n_gpu_layers: number; ctx_size: number }) => void
}

function ModelGuardrails({ modelPath, selectedDeviceIds, disabled, onSettingsChange }: ModelGuardrailsProps) {
  const [analysis, setAnalysis] = useState<ModelCheckResult | null>(null)
  const [checking, setChecking] = useState(false)
  const [checkError, setCheckError] = useState<string | null>(null)
  const [nGpuLayers, setNGpuLayers] = useState(-1)
  const [ctxSize, setCtxSize] = useState(4096)

  // Debounced fetch whenever model path or selected devices change
  useEffect(() => {
    if (!modelPath.trim()) {
      setAnalysis(null)
      setCheckError(null)
      return
    }
    const timer = setTimeout(async () => {
      setChecking(true)
      setCheckError(null)
      try {
        const result: ModelCheckResult = await api.modelCheck(modelPath.trim(), selectedDeviceIds)
        setAnalysis(result)
        setNGpuLayers(result.recommended_n_gpu_layers)
        setCtxSize(result.recommended_ctx_size)
        onSettingsChange({ n_gpu_layers: result.recommended_n_gpu_layers, ctx_size: result.recommended_ctx_size })
      } catch (e: unknown) {
        setCheckError(e instanceof Error ? e.message : String(e))
        setAnalysis(null)
      } finally {
        setChecking(false)
      }
    }, 800)
    return () => clearTimeout(timer)
  }, [modelPath, selectedDeviceIds, onSettingsChange])

  if (!modelPath.trim()) return null

  if (checking) {
    return (
      <div className="card flex items-center gap-2 text-xs text-muted py-3">
        <Loader2 size={13} className="animate-spin" />
        Analysing model memory requirements...
      </div>
    )
  }

  if (checkError) {
    return (
      <div className="card flex items-center gap-2 text-xs text-muted py-3">
        <AlertTriangle size={13} className="text-warning flex-shrink-0" />
        {checkError}
      </div>
    )
  }

  if (!analysis) return null

  const meta = FIT_META[analysis.fit_status]
  const maxMb = Math.max(analysis.total_available_mb, analysis.model_size_mb, 1)
  const modelPct = Math.min(100, (analysis.model_size_mb / maxMb) * 100)

  function updateLayers(v: number) {
    setNGpuLayers(v)
    onSettingsChange({ n_gpu_layers: v, ctx_size: ctxSize })
  }
  function updateCtx(v: number) {
    setCtxSize(v)
    onSettingsChange({ n_gpu_layers: nGpuLayers, ctx_size: v })
  }

  return (
    <div className="card space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-gray-300">Memory Analysis</h3>
        <span className={clsx('text-xs px-2 py-0.5 rounded-full font-medium', meta.badgeCls)}>
          {meta.label}
        </span>
      </div>

      {/* Memory bar */}
      <div>
        <div className="flex items-center justify-between text-xs text-muted mb-1.5">
          <span>Model: {fmt(analysis.model_size_mb)}</span>
          <span>Available: {fmt(analysis.total_available_mb)}</span>
        </div>
        <div className="h-2.5 bg-surface rounded-full overflow-hidden">
          <div
            className={clsx('h-full rounded-full transition-all duration-500', meta.barCls)}
            style={{ width: `${modelPct}%` }}
          />
        </div>
        <div className="flex items-center gap-4 mt-1.5 text-xs text-muted">
          <span>~{analysis.estimated_layers} layers</span>
          <span>Local free: {fmt(analysis.local_free_mb)}</span>
          {analysis.cluster_free_mb > 0 && (
            <span>Cluster: {fmt(analysis.cluster_free_mb)}</span>
          )}
        </div>
      </div>

      {/* Warnings */}
      {analysis.warnings.length > 0 && (
        <div className="space-y-1">
          {analysis.warnings.map((w, i) => (
            <div key={i} className="flex items-start gap-1.5 text-xs text-warning">
              <AlertTriangle size={11} className="flex-shrink-0 mt-0.5" />
              {w}
            </div>
          ))}
        </div>
      )}

      {/* GPU Layers slider */}
      <div>
        <div className="flex items-center justify-between mb-1">
          <label className="text-xs text-muted">GPU Layers</label>
          <span className="text-xs text-gray-300 font-mono">
            {nGpuLayers === -1
              ? `All (${analysis.estimated_layers})`
              : nGpuLayers === 0
              ? 'CPU only'
              : nGpuLayers}
          </span>
        </div>
        <input
          type="range"
          min={-1}
          max={analysis.estimated_layers}
          value={nGpuLayers}
          onChange={e => updateLayers(Number(e.target.value))}
          disabled={disabled}
          className="w-full accent-accent disabled:opacity-40"
        />
        <div className="flex justify-between text-xs text-muted mt-0.5">
          <span>CPU only</span>
          <span>All GPU</span>
        </div>
      </div>

      {/* Context size selector */}
      <div>
        <label className="block text-xs text-muted mb-1.5">Context Size</label>
        <div className="flex gap-1.5">
          {[2048, 4096, 8192, 16384].map(size => (
            <button
              key={size}
              onClick={() => updateCtx(size)}
              disabled={disabled}
              className={clsx(
                'flex-1 py-1 text-xs rounded-lg font-mono transition-colors disabled:opacity-40',
                ctxSize === size
                  ? 'bg-accent text-white'
                  : 'bg-surface text-muted border border-border hover:border-accent/50'
              )}
            >
              {size >= 1024 ? `${size / 1024}K` : size}
            </button>
          ))}
        </div>
      </div>
    </div>
  )
}

const BACKEND_TABS: { type: BackendType; label: string; defaultUrl: string }[] = [
  { type: 'llamacpp', label: 'llama.cpp', defaultUrl: '' },
  { type: 'lmstudio', label: 'LM Studio', defaultUrl: 'http://localhost:1234' },
  { type: 'vllm',     label: 'vLLM',      defaultUrl: 'http://localhost:8000' },
  { type: 'openai',   label: 'OpenAI',    defaultUrl: 'https://api.openai.com' },
  { type: 'custom',   label: 'Custom',    defaultUrl: 'http://localhost:8080' },
]

// ─── BackendSelector ─────────────────────────────────────────────────────────

interface BackendSelectorProps {
  activeConfig: BackendConfig | null
  onActivated: (cfg: BackendConfig) => void
}

function BackendSelector({ activeConfig, onActivated }: BackendSelectorProps) {
  const [selectedType, setSelectedType] = useState<BackendType>(
    (activeConfig?.backend_type as BackendType) ?? 'llamacpp'
  )
  const [url, setUrl] = useState(activeConfig?.url ?? '')
  const [model, setModel] = useState(activeConfig?.model ?? '')
  const [apiKey, setApiKey] = useState(activeConfig?.api_key ?? '')
  const [modelList, setModelList] = useState<string[]>([])
  const [modelLoading, setModelLoading] = useState(false)
  const [modelError, setModelError] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)
  const [showDropdown, setShowDropdown] = useState(false)

  // When tab changes, reset URL to the default for that backend
  function handleTabChange(type: BackendType) {
    setSelectedType(type)
    const tab = BACKEND_TABS.find(t => t.type === type)!
    // Keep current URL if it's non-empty and not a different default
    const isOtherDefault = BACKEND_TABS.some(t => t.type !== type && t.defaultUrl && t.defaultUrl === url)
    if (!url || isOtherDefault) {
      setUrl(tab.defaultUrl)
    }
    setModelList([])
    setModelError(null)
    setModel('')
  }

  async function fetchModels() {
    if (selectedType === 'llamacpp') return
    setModelLoading(true)
    setModelError(null)
    try {
      const list = await api.fetchBackendModels(selectedType, url, apiKey || undefined)
      setModelList(list)
      if (list.length > 0 && !list.includes(model)) {
        setModel(list[0])
      }
    } catch (e: unknown) {
      setModelError(e instanceof Error ? e.message : String(e))
      setModelList([])
    } finally {
      setModelLoading(false)
    }
  }

  async function handleActivate() {
    setSaving(true)
    setSaveError(null)
    try {
      const cfg: BackendConfig = {
        backend_type: selectedType,
        url: url.trim(),
        model: model.trim(),
        api_key: apiKey.trim() || undefined,
      }
      await api.setBackendConfig(cfg)
      onActivated(cfg)
    } catch (e: unknown) {
      setSaveError(e instanceof Error ? e.message : String(e))
    } finally {
      setSaving(false)
    }
  }

  const isActive =
    activeConfig?.backend_type === selectedType &&
    activeConfig?.url === url &&
    activeConfig?.model === model

  return (
    <div className="card space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold text-gray-300">Inference Backend</h2>
        {activeConfig && (
          <span className="text-xs text-muted">
            Active:{' '}
            <span className="text-accent font-medium">
              {BACKEND_TABS.find(t => t.type === activeConfig.backend_type)?.label ?? activeConfig.backend_type}
              {activeConfig.model ? ` / ${activeConfig.model}` : ''}
            </span>
          </span>
        )}
      </div>

      {/* Tab row */}
      <div className="flex flex-wrap gap-1">
        {BACKEND_TABS.map(tab => (
          <button
            key={tab.type}
            onClick={() => handleTabChange(tab.type)}
            className={clsx(
              'px-3 py-1.5 text-xs rounded-lg font-medium transition-colors',
              selectedType === tab.type
                ? 'bg-accent text-white'
                : 'bg-surface text-muted hover:text-gray-200 border border-border'
            )}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Per-backend config */}
      {selectedType === 'llamacpp' ? (
        <p className="text-xs text-muted">
          llama.cpp runs locally via the controls below. Use Start Inference to activate it.
        </p>
      ) : (
        <div className="space-y-3">
          {/* URL */}
          <div>
            <label className="block text-xs text-muted mb-1">
              {selectedType === 'openai' ? 'API Base URL' : 'Server URL'}
            </label>
            <input
              value={url}
              onChange={e => setUrl(e.target.value)}
              placeholder={BACKEND_TABS.find(t => t.type === selectedType)?.defaultUrl}
              className="w-full bg-surface border border-border rounded-lg px-3 py-2 text-sm text-gray-200 placeholder-muted focus:outline-none focus:border-accent font-mono"
            />
          </div>

          {/* API key (openai + custom) */}
          {(selectedType === 'openai' || selectedType === 'custom') && (
            <div>
              <label className="block text-xs text-muted mb-1">API Key</label>
              <input
                type="password"
                value={apiKey}
                onChange={e => setApiKey(e.target.value)}
                placeholder="sk-..."
                className="w-full bg-surface border border-border rounded-lg px-3 py-2 text-sm text-gray-200 placeholder-muted focus:outline-none focus:border-accent font-mono"
              />
            </div>
          )}

          {/* Model selector */}
          <div>
            <div className="flex items-center justify-between mb-1">
              <label className="text-xs text-muted">Model</label>
              <button
                onClick={fetchModels}
                disabled={modelLoading || !url.trim()}
                className="text-xs text-accent hover:underline disabled:opacity-40 flex items-center gap-1"
              >
                {modelLoading
                  ? <><Loader2 size={11} className="animate-spin" /> Loading...</>
                  : <><RefreshCw size={11} /> Fetch models</>}
              </button>
            </div>

            {modelList.length > 0 ? (
              <div className="relative">
                <button
                  onClick={() => setShowDropdown(v => !v)}
                  className="w-full flex items-center justify-between bg-surface border border-border rounded-lg px-3 py-2 text-sm text-gray-200 focus:outline-none focus:border-accent"
                >
                  <span className="truncate">{model || 'Select a model...'}</span>
                  <ChevronDown size={14} className="flex-shrink-0 text-muted" />
                </button>
                {showDropdown && (
                  <div className="absolute z-10 mt-1 w-full bg-surface-2 border border-border rounded-lg shadow-lg max-h-48 overflow-y-auto">
                    {modelList.map(m => (
                      <button
                        key={m}
                        onClick={() => { setModel(m); setShowDropdown(false) }}
                        className="w-full text-left px-3 py-2 text-sm hover:bg-surface text-gray-200 flex items-center gap-2"
                      >
                        {m === model && <Check size={12} className="text-accent flex-shrink-0" />}
                        <span className="truncate">{m}</span>
                      </button>
                    ))}
                  </div>
                )}
              </div>
            ) : (
              <input
                value={model}
                onChange={e => setModel(e.target.value)}
                placeholder="e.g. llama3.2, gpt-4o-mini"
                className="w-full bg-surface border border-border rounded-lg px-3 py-2 text-sm text-gray-200 placeholder-muted focus:outline-none focus:border-accent"
              />
            )}
            {modelError && (
              <p className="text-xs text-warning mt-1">{modelError} — enter model name manually.</p>
            )}
          </div>
        </div>
      )}

      {saveError && (
        <p className="text-xs text-danger">{saveError}</p>
      )}

      {selectedType !== 'llamacpp' && (
        <button
          onClick={handleActivate}
          disabled={saving || !url.trim()}
          className={clsx(
            'flex items-center gap-2 px-4 py-2 text-sm rounded-lg font-medium transition-colors disabled:opacity-40',
            isActive
              ? 'bg-success/15 text-success border border-success/30'
              : 'btn-primary'
          )}
        >
          {saving
            ? <Loader2 size={14} className="animate-spin" />
            : isActive
            ? <Check size={14} />
            : null}
          {isActive ? 'Active' : saving ? 'Saving...' : 'Activate'}
        </button>
      )}
    </div>
  )
}

// ─── Chat ─────────────────────────────────────────────────────────────────────

function ChatPanel({ inferenceRunning, activeConfig }: {
  inferenceRunning: boolean
  activeConfig: BackendConfig | null
}) {
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [input, setInput] = useState('')
  const [loading, setLoading] = useState(false)
  const [tokens, setTokens] = useState<number | null>(null)
  const endRef = useRef<HTMLDivElement>(null)

  // Chat is available when llamacpp inference is running OR an external backend is configured
  const externalBackendReady =
    activeConfig !== null &&
    activeConfig.backend_type !== 'llamacpp' &&
    activeConfig.url.trim() !== ''

  const chatReady = inferenceRunning || externalBackendReady

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  async function send() {
    if (!input.trim() || loading || !chatReady) return

    const userMsg: ChatMessage = { role: 'user', content: input.trim() }
    setMessages(prev => [...prev, userMsg])
    setInput('')
    setLoading(true)

    const history: ChatMessage[] = [...messages, userMsg]
    const t0 = Date.now()
    const modelName = activeConfig?.model || 'local'

    try {
      const resp = await api.chatCompletions(history, modelName)
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

  const placeholder = chatReady
    ? 'Type a message...'
    : 'Start llama.cpp inference or activate an external backend'

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
            {chatReady
              ? 'Start chatting with the model...'
              : placeholder}
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
          placeholder={placeholder}
          disabled={!chatReady || loading}
          className="flex-1 bg-surface border border-border rounded-lg px-3 py-2 text-sm text-gray-200 placeholder-muted disabled:opacity-50 focus:outline-none focus:border-accent"
        />
        <button
          onClick={send}
          disabled={!chatReady || loading || !input.trim()}
          className="btn-primary p-2 disabled:opacity-40"
        >
          <Send size={16} />
        </button>
      </div>
    </div>
  )
}

// ─── ModelPicker ──────────────────────────────────────────────────────────────

interface ModelPickerProps {
  value: string
  onChange: (path: string) => void
  disabled: boolean
}

interface ScannedModel {
  path: string
  name: string
  size_mb: number
}

interface HfRepo {
  id: string
  downloads?: number
  likes?: number
}

interface HfFile {
  filename: string
  size_bytes?: number
  download_url: string
}

function fmtMb(mb: number) {
  return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${mb} MB`
}
function fmtBytes(b: number) {
  const mb = b / (1024 * 1024)
  return fmtMb(mb)
}

type PickerTab = 'local' | 'huggingface'

function ModelPicker({ value, onChange, disabled }: ModelPickerProps) {
  const [tab, setTab] = useState<PickerTab>('local')

  // ── Local scan state ──
  const [scanning, setScanning] = useState(false)
  const [localModels, setLocalModels] = useState<ScannedModel[]>([])
  const [scanError, setScanError] = useState<string | null>(null)
  const [showLocalDropdown, setShowLocalDropdown] = useState(false)
  const localDropdownRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    function onClickOutside(e: MouseEvent) {
      if (localDropdownRef.current && !localDropdownRef.current.contains(e.target as Node)) {
        setShowLocalDropdown(false)
      }
    }
    document.addEventListener('mousedown', onClickOutside)
    return () => document.removeEventListener('mousedown', onClickOutside)
  }, [])

  async function handleScan() {
    setScanning(true)
    setScanError(null)
    try {
      const result = await api.scanLocalModels()
      setLocalModels(result.models)
      if (result.models.length > 0) setShowLocalDropdown(true)
      else setScanError('No .gguf models found. Download one from HuggingFace →')
    } catch (e: unknown) {
      setScanError(e instanceof Error ? e.message : String(e))
    } finally {
      setScanning(false)
    }
  }

  // ── HuggingFace state ──
  const [hfQuery, setHfQuery] = useState('')
  const [hfSearching, setHfSearching] = useState(false)
  const [hfRepos, setHfRepos] = useState<HfRepo[]>([])
  const [hfSearchError, setHfSearchError] = useState<string | null>(null)
  const [selectedRepo, setSelectedRepo] = useState<string | null>(null)
  const [hfFiles, setHfFiles] = useState<HfFile[]>([])
  const [hfFilesLoading, setHfFilesLoading] = useState(false)
  const [hfFilesError, setHfFilesError] = useState<string | null>(null)

  // Download state
  const [downloadingFile, setDownloadingFile] = useState<string | null>(null)
  const [downloadProgress, setDownloadProgress] = useState(0)
  const [downloadedMb, setDownloadedMb] = useState(0)
  const [totalMb, setTotalMb] = useState(0)
  const [downloadError, setDownloadError] = useState<string | null>(null)

  async function handleHfSearch(e: React.FormEvent) {
    e.preventDefault()
    if (!hfQuery.trim()) return
    setHfSearching(true)
    setHfSearchError(null)
    setHfRepos([])
    setSelectedRepo(null)
    setHfFiles([])
    try {
      const result = await api.hfSearch(hfQuery.trim())
      setHfRepos(result.models)
      if (result.models.length === 0) setHfSearchError('No GGUF repos found. Try a different search term.')
    } catch (e: unknown) {
      setHfSearchError(e instanceof Error ? e.message : String(e))
    } finally {
      setHfSearching(false)
    }
  }

  async function handleSelectRepo(repo: string) {
    setSelectedRepo(repo)
    setHfFiles([])
    setHfFilesError(null)
    setHfFilesLoading(true)
    try {
      const result = await api.hfListFiles(repo)
      setHfFiles(result.files)
      if (result.files.length === 0) setHfFilesError('No .gguf files found in this repository.')
    } catch (e: unknown) {
      setHfFilesError(e instanceof Error ? e.message : String(e))
    } finally {
      setHfFilesLoading(false)
    }
  }

  async function handleDownload(file: HfFile) {
    if (!selectedRepo) return
    setDownloadingFile(file.filename)
    setDownloadProgress(0)
    setDownloadedMb(0)
    setTotalMb(0)
    setDownloadError(null)
    try {
      const resp = await api.hfDownload(selectedRepo, file.filename)
      if (!resp.ok) {
        const err = await resp.json().catch(() => ({ error: `HTTP ${resp.status}` }))
        setDownloadError(err.error ?? `HTTP ${resp.status}`)
        return
      }
      const reader = resp.body?.getReader()
      if (!reader) { setDownloadError('No response stream'); return }
      const decoder = new TextDecoder()
      let buf = ''
      while (true) {
        const { done, value } = await reader.read()
        if (done) break
        buf += decoder.decode(value, { stream: true })
        const lines = buf.split('\n')
        buf = lines.pop() ?? ''
        for (const line of lines) {
          const trimmed = line.trim()
          if (!trimmed) continue
          try {
            const msg = JSON.parse(trimmed)
            if (msg.error) { setDownloadError(msg.error); reader.cancel().catch(() => {}); return }
            if (msg.progress !== undefined) {
              setDownloadProgress(msg.progress)
              setDownloadedMb(msg.downloaded_mb ?? 0)
              setTotalMb(msg.total_mb ?? 0)
            }
            if (msg.done && msg.path) {
              onChange(msg.path)
              setTab('local')
            }
          } catch { /* ignore non-JSON */ }
        }
      }
    } catch (e: unknown) {
      setDownloadError(e instanceof Error ? e.message : String(e))
    } finally {
      setDownloadingFile(null)
    }
  }

  return (
    <div className="space-y-3">
      {/* Path input + Scan */}
      <div className="flex gap-2">
        <input
          value={value}
          onChange={e => onChange(e.target.value)}
          placeholder="/path/to/model.gguf"
          disabled={disabled}
          className="flex-1 bg-surface border border-border rounded-lg px-3 py-2 text-sm text-gray-200 placeholder-muted disabled:opacity-50 focus:outline-none focus:border-accent font-mono"
        />
        <button
          onClick={handleScan}
          disabled={disabled || scanning}
          title="Scan common directories for .gguf models"
          className="flex items-center gap-1.5 px-3 py-2 text-xs rounded-lg bg-surface border border-border text-muted hover:text-gray-200 hover:border-accent/50 disabled:opacity-40 transition-colors whitespace-nowrap"
        >
          {scanning ? <Loader2 size={13} className="animate-spin" /> : <FolderSearch size={13} />}
          {scanning ? 'Scanning...' : 'Scan'}
        </button>
      </div>

      {/* Local scan results dropdown */}
      {showLocalDropdown && localModels.length > 0 && (
        <div ref={localDropdownRef} className="w-full bg-panel border border-border rounded-lg shadow-lg max-h-48 overflow-y-auto">
          <p className="px-3 py-1.5 text-xs text-muted border-b border-border">
            {localModels.length} local model{localModels.length !== 1 ? 's' : ''} found
          </p>
          {localModels.map(m => (
            <button key={m.path} onClick={() => { onChange(m.path); setShowLocalDropdown(false) }}
              className={clsx('w-full text-left px-3 py-2 text-sm hover:bg-surface transition-colors flex items-center justify-between gap-3', value === m.path ? 'text-accent' : 'text-gray-200')}
            >
              <div className="min-w-0">
                <p className="truncate font-medium">{m.name}</p>
                <p className="text-xs text-muted truncate font-mono">{m.path}</p>
              </div>
              <div className="flex items-center gap-2 flex-shrink-0">
                <span className="text-xs text-muted">{fmtMb(m.size_mb)}</span>
                {value === m.path && <Check size={13} className="text-accent" />}
              </div>
            </button>
          ))}
        </div>
      )}

      {scanError && (
        <p className="text-xs text-warning flex items-center gap-1.5">
          <AlertTriangle size={11} className="flex-shrink-0" />
          {scanError}
          {scanError.includes('HuggingFace') && (
            <button onClick={() => setTab('huggingface')} className="text-accent hover:underline">
              Search HuggingFace →
            </button>
          )}
        </p>
      )}

      {/* HuggingFace section toggle */}
      <div className="border-t border-border pt-2">
        <button
          onClick={() => setTab(tab === 'huggingface' ? 'local' : 'huggingface')}
          className="flex items-center gap-1.5 text-xs text-accent hover:underline"
        >
          <Download size={12} />
          {tab === 'huggingface' ? 'Hide HuggingFace search' : 'Search & download from HuggingFace'}
        </button>
      </div>

      {tab === 'huggingface' && (
        <div className="space-y-3 border border-border rounded-lg p-3 bg-surface/50">
          <p className="text-xs text-muted">Search for GGUF models on HuggingFace Hub. Models will be saved to <code className="font-mono text-gray-300">~/.sharedmem/models/</code></p>

          {/* Search form */}
          <form onSubmit={handleHfSearch} className="flex gap-2">
            <input
              value={hfQuery}
              onChange={e => setHfQuery(e.target.value)}
              placeholder="e.g. Llama-3.2, Mistral, Qwen2.5..."
              className="flex-1 bg-surface border border-border rounded-lg px-3 py-1.5 text-sm text-gray-200 placeholder-muted focus:outline-none focus:border-accent"
            />
            <button
              type="submit"
              disabled={hfSearching || !hfQuery.trim()}
              className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg btn-primary disabled:opacity-40 whitespace-nowrap"
            >
              {hfSearching ? <Loader2 size={12} className="animate-spin" /> : null}
              {hfSearching ? 'Searching...' : 'Search'}
            </button>
          </form>

          {hfSearchError && (
            <p className="text-xs text-warning flex items-center gap-1"><AlertTriangle size={11} />{hfSearchError}</p>
          )}

          {/* Repo results */}
          {hfRepos.length > 0 && (
            <div className="space-y-1 max-h-40 overflow-y-auto">
              <p className="text-xs text-muted mb-1">{hfRepos.length} repos — click to see files</p>
              {hfRepos.map(repo => (
                <button
                  key={repo.id}
                  onClick={() => handleSelectRepo(repo.id)}
                  className={clsx(
                    'w-full text-left px-3 py-2 rounded-lg text-sm transition-colors flex items-center justify-between gap-2',
                    selectedRepo === repo.id
                      ? 'bg-accent/10 border border-accent/30 text-accent'
                      : 'bg-surface hover:bg-surface/80 border border-transparent text-gray-200'
                  )}
                >
                  <span className="truncate font-mono text-xs">{repo.id}</span>
                  <div className="flex items-center gap-2 text-xs text-muted flex-shrink-0">
                    {repo.downloads !== undefined && <span>↓ {(repo.downloads / 1000).toFixed(0)}k</span>}
                    {repo.likes !== undefined && <span>♥ {repo.likes}</span>}
                    {selectedRepo === repo.id && <ChevronDown size={12} />}
                  </div>
                </button>
              ))}
            </div>
          )}

          {/* Files in selected repo */}
          {selectedRepo && (
            <div className="border-t border-border pt-2 space-y-1">
              <p className="text-xs text-muted font-medium">{selectedRepo} — choose a quantisation:</p>
              {hfFilesLoading && (
                <div className="flex items-center gap-2 text-xs text-muted py-2">
                  <Loader2 size={12} className="animate-spin" />Loading files...
                </div>
              )}
              {hfFilesError && <p className="text-xs text-warning">{hfFilesError}</p>}
              {hfFiles.length > 0 && (
                <div className="space-y-1 max-h-48 overflow-y-auto">
                  {hfFiles.map(file => {
                    const isDownloading = downloadingFile === file.filename
                    return (
                      <div key={file.filename} className="flex items-center gap-2 rounded-lg border border-border p-2 bg-surface">
                        <div className="flex-1 min-w-0">
                          <p className="text-xs font-mono text-gray-200 truncate">{file.filename}</p>
                          {file.size_bytes && (
                            <p className="text-xs text-muted">{fmtBytes(file.size_bytes)}</p>
                          )}
                          {isDownloading && (
                            <div className="mt-1">
                              <div className="h-1.5 bg-surface rounded-full overflow-hidden">
                                <div
                                  className="h-full bg-accent rounded-full transition-all duration-300"
                                  style={{ width: `${downloadProgress}%` }}
                                />
                              </div>
                              <p className="text-xs text-muted mt-0.5">
                                {downloadProgress}% — {fmtMb(downloadedMb)} / {fmtMb(totalMb)}
                              </p>
                            </div>
                          )}
                        </div>
                        <button
                          onClick={() => handleDownload(file)}
                          disabled={!!downloadingFile || disabled}
                          className={clsx(
                            'flex items-center gap-1 px-2 py-1 text-xs rounded-lg transition-colors disabled:opacity-40 flex-shrink-0',
                            isDownloading
                              ? 'bg-accent/20 text-accent'
                              : 'bg-accent text-white hover:bg-accent/90'
                          )}
                        >
                          {isDownloading
                            ? <><Loader2 size={11} className="animate-spin" />Downloading</>
                            : <><Download size={11} />Download</>}
                        </button>
                      </div>
                    )
                  })}
                </div>
              )}
              {downloadError && (
                <p className="text-xs text-danger flex items-center gap-1"><AlertTriangle size={11} />{downloadError}</p>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  )
}

// ─── Main Page ────────────────────────────────────────────────────────────────

export function InferencePage() {
  const [clusterStatus, setClusterStatus] = useState<ClusterStatus | null>(null)
  const [selectedDeviceIds, setSelectedDeviceIds] = useState<string[]>([])
  const [modelPath, setModelPath] = useState('')
  const [inferenceSettings, setInferenceSettings] = useState({ n_gpu_layers: -1, ctx_size: 4096 })
  const [loading, setLoading] = useState(false)
  const [actionError, setActionError] = useState<string | null>(null)
  const [refreshing, setRefreshing] = useState(false)
  const [activeBackend, setActiveBackend] = useState<BackendConfig | null>(null)

  const refresh = useCallback(async () => {
    setRefreshing(true)
    try {
      const [data, cfg] = await Promise.all([
        api.clusterStatus(),
        api.backendConfig().catch(() => null),
      ])
      setClusterStatus(data)
      if (cfg) setActiveBackend(cfg)
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

  // Clear stale action errors whenever the model path changes
  useEffect(() => {
    setActionError(null)
  }, [modelPath])

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
      await api.startInference(
        modelPath.trim(),
        selectedDeviceIds,
        inferenceSettings.n_gpu_layers,
        inferenceSettings.ctx_size,
      )
      // Auto-activate llamacpp backend when inference starts
      const cfg: BackendConfig = { backend_type: 'llamacpp', url: '', model: modelPath.trim() }
      await api.setBackendConfig(cfg)
      setActiveBackend(cfg)
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

  const [installing, setInstalling] = useState(false)
  const [installStatus, setInstallStatus] = useState('')
  const [installError, setInstallError] = useState<string | null>(null)

  async function handleInstallBinaries() {
    setInstalling(true)
    setInstallStatus('Starting...')
    setInstallError(null)
    try {
      const resp = await api.installBinaries()
      if (!resp.ok) {
        const err = await resp.json().catch(() => ({ error: `HTTP ${resp.status}` }))
        setInstallError(err.error ?? `HTTP ${resp.status}`)
        return
      }
      const reader = resp.body?.getReader()
      if (!reader) {
        setInstallError('No response body')
        return
      }
      const decoder = new TextDecoder()
      let buf = ''
      while (true) {
        const { done, value } = await reader.read()
        if (done) break
        buf += decoder.decode(value, { stream: true })
        const lines = buf.split('\n')
        buf = lines.pop() ?? ''
        for (const line of lines) {
          const trimmed = line.trim()
          if (!trimmed) continue
          try {
            const msg = JSON.parse(trimmed)
            if (msg.status) setInstallStatus(msg.status)
            if (msg.error) {
              setInstallError(msg.error)
              // Cancel the stream so the server-side connection is released.
              reader.cancel().catch(() => {})
              return
            }
            if (msg.done) {
              setInstallStatus('Done!')
              await refresh()
            }
          } catch {
            // ignore non-JSON lines
          }
        }
      }
    } catch (e: unknown) {
      setInstallError(e instanceof Error ? e.message : String(e))
    } finally {
      setInstalling(false)
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
            Run LLM inference locally with llama.cpp or connect to an external backend
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

      {/* Backend selector — always visible at top */}
      <BackendSelector
        activeConfig={activeBackend}
        onActivated={cfg => setActiveBackend(cfg)}
      />

      {/* llama.cpp section — shown when llamacpp tab or no external backend active */}
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
          <div className="mt-3 space-y-2">
            <p className="text-xs text-warning">
              llama.cpp binaries not found in PATH or{' '}
              <code className="font-mono">~/.sharedmem/bin/</code>.
            </p>
            <div className="flex items-center gap-3 flex-wrap">
              <button
                onClick={handleInstallBinaries}
                disabled={installing}
                className="btn-primary flex items-center gap-1.5 text-xs px-3 py-1.5 disabled:opacity-50"
              >
                {installing
                  ? <Loader2 size={13} className="animate-spin" />
                  : <Download size={13} />}
                {installing ? 'Installing...' : 'Install Automatically'}
              </button>
              <a
                href="https://github.com/ggml-org/llama.cpp/releases"
                target="_blank"
                rel="noopener noreferrer"
                className="text-xs text-accent hover:underline"
              >
                Manual Download
              </a>
            </div>
            {installStatus && !installError && (
              <p className="text-xs text-accent font-mono animate-pulse">{installStatus}</p>
            )}
            {installError && (
              <p className="text-xs text-danger">{installError}</p>
            )}
          </div>
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
            <h2 className="text-sm font-semibold text-gray-300 mb-3">Model (llama.cpp)</h2>
            <ModelPicker
              value={modelPath}
              onChange={setModelPath}
              disabled={inferenceRunning}
            />
          </div>

          {/* Device selection */}
          <div className="card">
            <h2 className="text-sm font-semibold text-gray-300 mb-3">
              Network Devices{' '}
              <span className="text-muted font-normal">
                ({clusterStatus?.devices.length ?? 0} approved)
              </span>
            </h2>
            {clusterStatus === null ? (
              <p className="text-xs text-muted">Loading devices...</p>
            ) : clusterStatus.devices.length === 0 ? (
              <p className="text-xs text-muted">
                No approved devices. Go to{' '}
                <Link to="/devices" className="text-accent hover:underline">
                  Devices
                </Link>{' '}
                to approve network machines, then have them run the agent (see{' '}
                <Link to="/agent" className="text-accent hover:underline">
                  Agent
                </Link>
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
                        'flex items-center gap-3 p-2 rounded-lg border transition-colors',
                        ready ? 'cursor-pointer' : 'cursor-default opacity-60',
                        selectedDeviceIds.includes(device.id)
                          ? 'border-accent/40 bg-accent/5'
                          : 'border-border hover:border-border/80'
                      )}
                    >
                      <input
                        type="checkbox"
                        checked={selectedDeviceIds.includes(device.id)}
                        onChange={() => ready && !inferenceRunning && toggleDevice(device.id)}
                        disabled={inferenceRunning || !ready}
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
                        {!ready && (
                          <p className="text-xs text-warning mt-0.5 ml-5">
                            RPC unreachable — is llama-rpc-server running on this device?
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

          {/* Model memory guardrails — auto-fetched when model path is set */}
          <ModelGuardrails
            modelPath={modelPath}
            selectedDeviceIds={selectedDeviceIds}
            disabled={inferenceRunning}
            onSettingsChange={setInferenceSettings}
          />

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
          <ChatPanel inferenceRunning={inferenceRunning} activeConfig={activeBackend} />

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
