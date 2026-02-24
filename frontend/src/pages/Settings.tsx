import { useState, useEffect } from 'react'
import { Save, Info } from 'lucide-react'
import type { Settings } from '../types'
import { api } from '../lib/api'

const SETTING_LABELS: Record<string, { label: string; description: string; type: 'boolean' | 'string' | 'number' }> = {
  trust_local_network: {
    label: 'Trust local network',
    description: 'Auto-approve all mDNS-discovered devices without manual approval',
    type: 'boolean',
  },
  auto_start_ollama: {
    label: 'Auto-start Ollama',
    description: 'Automatically start Ollama on server launch',
    type: 'boolean',
  },
  mdns_enabled: {
    label: 'Enable mDNS discovery',
    description: 'Broadcast this host and scan for other SharedMem devices on the LAN',
    type: 'boolean',
  },
  api_port: {
    label: 'API port',
    description: 'Port the backend server listens on (requires restart)',
    type: 'number',
  },
  ollama_host: {
    label: 'Ollama host',
    description: 'URL where Ollama is (or will be) running',
    type: 'string',
  },
  default_role: {
    label: 'Default role ID',
    description: 'Role assigned to auto-approved devices',
    type: 'string',
  },
}

interface SettingsPageProps {
  settings: Settings
  onSettingsChange: (s: Settings) => void
}

export function SettingsPage({ settings, onSettingsChange }: SettingsPageProps) {
  const [local, setLocal] = useState<Settings>(settings)
  const [saved, setSaved] = useState<Record<string, boolean>>({})

  useEffect(() => { setLocal(settings) }, [settings])

  const save = async (key: string, value: string) => {
    await api.updateSetting(key, value)
    setSaved(s => ({ ...s, [key]: true }))
    onSettingsChange({ ...local, [key]: value })
    setTimeout(() => setSaved(s => ({ ...s, [key]: false })), 2000)
  }

  return (
    <div className="p-6 space-y-6">
      <div>
        <h1 className="text-xl font-bold text-gray-100">Settings</h1>
        <p className="text-sm text-muted mt-0.5">Server configuration (persisted in SQLite)</p>
      </div>

      <div className="space-y-3">
        {Object.entries(SETTING_LABELS).map(([key, meta]) => {
          const val = local[key] ?? ''
          return (
            <div key={key} className="card space-y-2">
              <div className="flex items-start justify-between gap-4">
                <div>
                  <p className="text-sm font-semibold text-gray-100">{meta.label}</p>
                  <p className="text-xs text-muted mt-0.5 flex items-center gap-1">
                    <Info size={11} /> {meta.description}
                  </p>
                </div>
                {saved[key] && (
                  <span className="badge-approved text-xs flex-shrink-0">Saved</span>
                )}
              </div>

              {meta.type === 'boolean' ? (
                <div className="flex items-center gap-3">
                  <button
                    onClick={() => save(key, val === 'true' ? 'false' : 'true')}
                    className={val === 'true' ? 'btn-success text-xs' : 'btn-ghost text-xs'}
                  >
                    {val === 'true' ? 'Enabled' : 'Disabled'}
                  </button>
                </div>
              ) : (
                <div className="flex gap-2 max-w-md">
                  <input
                    type={meta.type === 'number' ? 'number' : 'text'}
                    className="input text-sm font-mono"
                    value={val}
                    onChange={e => setLocal(l => ({ ...l, [key]: e.target.value }))}
                    onKeyDown={e => e.key === 'Enter' && save(key, val)}
                  />
                  <button
                    onClick={() => save(key, val)}
                    className="btn-primary text-sm flex-shrink-0"
                  >
                    <Save size={14} />
                  </button>
                </div>
              )}
            </div>
          )
        })}
      </div>

      {/* About */}
      <div className="card border-border/50 bg-surface/50">
        <p className="text-xs text-muted text-center">
          SharedMem Network v0.1 &mdash; Rust (Axum) backend + React frontend
        </p>
      </div>
    </div>
  )
}
