import { useState } from 'react'
import { Plus, RefreshCw, Search } from 'lucide-react'
import type { Device, Role } from '../types'
import { DeviceCard } from '../components/DeviceCard'

interface DevicesPageProps {
  devices: Device[]
  roles: Role[]
  loading: boolean
  onRefresh: () => void
  onApprove: (id: string, roleId?: string) => void
  onDeny: (id: string) => void
  onAllocate: (id: string, mb: number) => void
  onRemove: (id: string) => void
  onAdd: (name: string, ip: string, mac?: string) => void
}

const FILTERS = ['all', 'pending', 'approved', 'denied', 'offline'] as const

export function DevicesPage({
  devices, roles, loading, onRefresh, onApprove, onDeny, onAllocate, onRemove, onAdd
}: DevicesPageProps) {
  const [filter, setFilter] = useState<typeof FILTERS[number]>('all')
  const [search, setSearch] = useState('')
  const [showAdd, setShowAdd] = useState(false)
  const [addForm, setAddForm] = useState({ name: '', ip: '', mac: '' })
  const [adding, setAdding] = useState(false)

  const filtered = devices.filter(d => {
    const matchStatus = filter === 'all' || d.status === filter
    const q = search.toLowerCase()
    const matchSearch = !q ||
      d.name.toLowerCase().includes(q) ||
      d.ip.includes(q) ||
      (d.hostname ?? '').toLowerCase().includes(q)
    return matchStatus && matchSearch
  })

  const counts = {
    all: devices.length,
    pending: devices.filter(d => d.status === 'pending').length,
    approved: devices.filter(d => d.status === 'approved').length,
    denied: devices.filter(d => d.status === 'denied').length,
    offline: devices.filter(d => d.status === 'offline').length,
  }

  const handleAdd = async () => {
    if (!addForm.name || !addForm.ip) return
    setAdding(true)
    try {
      await onAdd(addForm.name, addForm.ip, addForm.mac || undefined)
      setShowAdd(false)
      setAddForm({ name: '', ip: '', mac: '' })
    } finally {
      setAdding(false)
    }
  }

  return (
    <div className="p-6 space-y-5">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold text-gray-100">Devices</h1>
          <p className="text-sm text-muted mt-0.5">{devices.length} device{devices.length !== 1 ? 's' : ''} known</p>
        </div>
        <div className="flex gap-2">
          <button onClick={onRefresh} className="btn-ghost text-sm" disabled={loading}>
            <RefreshCw size={15} className={loading ? 'animate-spin' : ''} />
          </button>
          <button onClick={() => setShowAdd(v => !v)} className="btn-primary text-sm">
            <Plus size={15} /> Add Device
          </button>
        </div>
      </div>

      {/* Manual add form */}
      {showAdd && (
        <div className="card border-accent/40 space-y-3">
          <p className="text-sm font-semibold text-gray-100">Add Device Manually</p>
          <div className="grid grid-cols-3 gap-3">
            <div>
              <label className="block text-xs text-muted mb-1">Name</label>
              <input className="input text-sm" placeholder="My Laptop" value={addForm.name}
                onChange={e => setAddForm(f => ({ ...f, name: e.target.value }))} />
            </div>
            <div>
              <label className="block text-xs text-muted mb-1">IP Address</label>
              <input className="input text-sm font-mono" placeholder="192.168.1.100" value={addForm.ip}
                onChange={e => setAddForm(f => ({ ...f, ip: e.target.value }))} />
            </div>
            <div>
              <label className="block text-xs text-muted mb-1">MAC (optional)</label>
              <input className="input text-sm font-mono" placeholder="aa:bb:cc:dd:ee:ff" value={addForm.mac}
                onChange={e => setAddForm(f => ({ ...f, mac: e.target.value }))} />
            </div>
          </div>
          <div className="flex gap-2">
            <button onClick={handleAdd} disabled={adding || !addForm.name || !addForm.ip} className="btn-primary text-sm">
              {adding ? 'Adding...' : 'Add & Request Approval'}
            </button>
            <button onClick={() => setShowAdd(false)} className="btn-ghost text-sm">Cancel</button>
          </div>
        </div>
      )}

      {/* Filters + search */}
      <div className="flex items-center gap-3 flex-wrap">
        <div className="flex bg-panel border border-border rounded-lg overflow-hidden">
          {FILTERS.map(f => (
            <button
              key={f}
              onClick={() => setFilter(f)}
              className={`px-3 py-1.5 text-xs font-medium transition-colors ${
                filter === f ? 'bg-accent text-white' : 'text-muted hover:text-gray-300'
              }`}
            >
              {f} {counts[f] > 0 && <span className="ml-1 opacity-70">{counts[f]}</span>}
            </button>
          ))}
        </div>
        <div className="relative flex-1 max-w-xs">
          <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-muted" />
          <input
            className="input text-sm pl-8"
            placeholder="Search devices..."
            value={search}
            onChange={e => setSearch(e.target.value)}
          />
        </div>
      </div>

      {/* Device grid */}
      {filtered.length === 0 ? (
        <div className="card text-center text-muted text-sm py-12">
          {devices.length === 0
            ? 'No devices found. Waiting for auto-discovery or add one manually.'
            : 'No devices match your filter.'}
        </div>
      ) : (
        <div className="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-4">
          {filtered.map(device => (
            <DeviceCard
              key={device.id}
              device={device}
              roles={roles}
              onApprove={onApprove}
              onDeny={onDeny}
              onAllocate={onAllocate}
              onRemove={onRemove}
            />
          ))}
        </div>
      )}
    </div>
  )
}
