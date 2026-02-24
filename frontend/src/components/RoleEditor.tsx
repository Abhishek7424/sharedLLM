import { useState } from 'react'
import { Plus, Save, Trash2, Shield } from 'lucide-react'
import type { Role } from '../types'
import { api } from '../lib/api'

interface RoleEditorProps {
  roles: Role[]
  onRefresh: () => void
}

function fmt(mb: number) {
  return mb >= 1024 ? `${(mb / 1024).toFixed(0)} GB` : `${mb} MB`
}

const BUILT_IN = ['role-admin', 'role-user', 'role-guest']

export function RoleEditor({ roles, onRefresh }: RoleEditorProps) {
  const [creating, setCreating] = useState(false)
  const [form, setForm] = useState({ name: '', max_memory_mb: 2048, can_pull_models: false, trust_level: 1 })
  const [saving, setSaving] = useState(false)

  const handleCreate = async () => {
    setSaving(true)
    try {
      await api.createRole(form)
      setCreating(false)
      setForm({ name: '', max_memory_mb: 2048, can_pull_models: false, trust_level: 1 })
      onRefresh()
    } finally {
      setSaving(false)
    }
  }

  const handleDelete = async (id: string) => {
    if (!confirm('Delete this role?')) return
    await api.deleteRole(id)
    onRefresh()
  }

  const trustLabel = (level: number) =>
    level >= 3 ? 'Admin' : level === 2 ? 'Trusted' : 'Guest'

  return (
    <div className="space-y-3">
      {roles.map(role => (
        <div key={role.id} className="card flex items-center gap-4">
          <div className="w-9 h-9 rounded-lg bg-accent/15 text-accent flex items-center justify-center flex-shrink-0">
            <Shield size={18} />
          </div>
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <span className="font-semibold text-sm text-gray-100">{role.name}</span>
              {BUILT_IN.includes(role.id) && (
                <span className="text-xs bg-surface border border-border text-muted px-1.5 py-0.5 rounded">built-in</span>
              )}
            </div>
            <div className="flex gap-4 text-xs text-muted mt-0.5">
              <span>Max: {fmt(role.max_memory_mb)}</span>
              <span>Trust: {trustLabel(role.trust_level)}</span>
              <span>{role.can_pull_models ? 'Can pull models' : 'No pull'}</span>
            </div>
          </div>
          {!BUILT_IN.includes(role.id) && (
            <button onClick={() => handleDelete(role.id)} className="btn-danger py-1.5 px-2.5 text-xs">
              <Trash2 size={13} />
            </button>
          )}
        </div>
      ))}

      {/* Create new role */}
      {creating ? (
        <div className="card space-y-3 border-accent/40">
          <p className="text-sm font-semibold text-gray-100">New Role</p>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs text-muted mb-1">Name</label>
              <input className="input text-sm" value={form.name} onChange={e => setForm(f => ({ ...f, name: e.target.value }))} />
            </div>
            <div>
              <label className="block text-xs text-muted mb-1">Max Memory (MB)</label>
              <input type="number" className="input text-sm" value={form.max_memory_mb} step={512} min={256}
                onChange={e => setForm(f => ({ ...f, max_memory_mb: Number(e.target.value) }))} />
            </div>
            <div>
              <label className="block text-xs text-muted mb-1">Trust Level (1-3)</label>
              <input type="number" className="input text-sm" value={form.trust_level} min={1} max={3}
                onChange={e => setForm(f => ({ ...f, trust_level: Number(e.target.value) }))} />
            </div>
            <div className="flex items-center gap-2 mt-4">
              <input type="checkbox" id="pull" checked={form.can_pull_models}
                onChange={e => setForm(f => ({ ...f, can_pull_models: e.target.checked }))}
                className="w-4 h-4 rounded accent-accent" />
              <label htmlFor="pull" className="text-sm text-gray-300">Can pull models</label>
            </div>
          </div>
          <div className="flex gap-2">
            <button onClick={handleCreate} disabled={saving || !form.name} className="btn-primary text-sm">
              <Save size={14} /> {saving ? 'Saving...' : 'Create'}
            </button>
            <button onClick={() => setCreating(false)} className="btn-ghost text-sm">Cancel</button>
          </div>
        </div>
      ) : (
        <button onClick={() => setCreating(true)} className="btn-ghost w-full text-sm border-dashed">
          <Plus size={16} /> Add Role
        </button>
      )}
    </div>
  )
}
