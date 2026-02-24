import { Monitor, HardDrive, Clock, Cpu } from 'lucide-react'
import type { Device, MemorySnapshot, OllamaModel } from '../types'
import { MemoryBar } from '../components/MemoryBar'

interface DashboardProps {
  devices: Device[]
  snapshots: MemorySnapshot[]
  ollamaRunning: boolean
  ollamaHost?: string
  models: OllamaModel[]
}

function StatCard({ icon: Icon, label, value, sub, color = 'text-accent' }: {
  icon: React.ElementType, label: string, value: string | number, sub?: string, color?: string
}) {
  return (
    <div className="card flex items-center gap-4">
      <div className={`w-10 h-10 rounded-xl bg-surface flex items-center justify-center ${color}`}>
        <Icon size={20} />
      </div>
      <div>
        <p className="text-2xl font-bold text-gray-100">{value}</p>
        <p className="text-xs text-muted">{label}</p>
        {sub && <p className="text-xs text-gray-400 mt-0.5">{sub}</p>}
      </div>
    </div>
  )
}

export function Dashboard({ devices, snapshots, ollamaRunning, models }: DashboardProps) {
  const approved = devices.filter(d => d.status === 'approved').length
  const pending = devices.filter(d => d.status === 'pending').length
  const totalAllocated = devices.reduce((s, d) => s + d.allocated_memory_mb, 0)

  function fmt(mb: number) {
    return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${mb} MB`
  }

  return (
    <div className="p-6 space-y-6">
      <div>
        <h1 className="text-xl font-bold text-gray-100">Dashboard</h1>
        <p className="text-sm text-muted mt-0.5">System overview and memory status</p>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-2 xl:grid-cols-4 gap-4">
        <StatCard icon={Monitor} label="Connected devices" value={approved} color="text-accent" />
        <StatCard icon={Clock} label="Pending approval" value={pending} color="text-warning" />
        <StatCard icon={HardDrive} label="Total allocated" value={fmt(totalAllocated)} color="text-success" />
        <StatCard
          icon={Cpu}
          label="Ollama"
          value={ollamaRunning ? 'Running' : 'Offline'}
          sub={models.length ? `${models.length} model${models.length !== 1 ? 's' : ''}` : undefined}
          color={ollamaRunning ? 'text-success' : 'text-danger'}
        />
      </div>

      {/* Memory pools */}
      <div>
        <h2 className="text-sm font-semibold text-gray-300 mb-3">Memory Pools</h2>
        {snapshots.length === 0 ? (
          <div className="card text-center text-muted text-sm py-8">Detecting memory providers...</div>
        ) : (
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
            {snapshots.map(snap => (
              <div key={snap.provider_id} className="card">
                <MemoryBar snapshot={snap} />
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Recent devices */}
      {devices.length > 0 && (
        <div>
          <h2 className="text-sm font-semibold text-gray-300 mb-3">Recent Devices</h2>
          <div className="card overflow-hidden p-0">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-border bg-surface/50">
                  <th className="text-left text-xs text-muted font-medium px-4 py-2.5">Name</th>
                  <th className="text-left text-xs text-muted font-medium px-4 py-2.5">IP</th>
                  <th className="text-left text-xs text-muted font-medium px-4 py-2.5">Status</th>
                  <th className="text-left text-xs text-muted font-medium px-4 py-2.5">Memory</th>
                </tr>
              </thead>
              <tbody>
                {devices.slice(0, 5).map(d => (
                  <tr key={d.id} className="border-b border-border/50 last:border-0 hover:bg-white/[0.02]">
                    <td className="px-4 py-2.5 text-gray-200 font-medium">{d.name}</td>
                    <td className="px-4 py-2.5 text-muted font-mono text-xs">{d.ip}</td>
                    <td className="px-4 py-2.5">
                      <span className={`badge-${d.status}`}>{d.status}</span>
                    </td>
                    <td className="px-4 py-2.5 text-xs text-muted">
                      {d.allocated_memory_mb > 0 ? fmt(d.allocated_memory_mb) : '—'}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  )
}
