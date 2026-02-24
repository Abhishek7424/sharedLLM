import { Monitor, HardDrive, Clock, Cpu, Wifi, WifiOff } from 'lucide-react'
import { clsx } from 'clsx'
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

function fmt(mb: number) {
  return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${mb} MB`
}

export function Dashboard({ devices, snapshots, ollamaRunning, models }: DashboardProps) {
  const approved = devices.filter(d => d.status === 'approved')
  const pending = devices.filter(d => d.status === 'pending').length
  const totalAllocated = devices.reduce((s, d) => s + d.allocated_memory_mb, 0)

  // Devices that have reported memory (via RPC probe)
  const clusterDevices = approved.filter(d => d.memory_total_mb > 0)
  const clusterTotalMb = clusterDevices.reduce((s, d) => s + d.memory_total_mb, 0)

  return (
    <div className="p-6 space-y-6">
      <div>
        <h1 className="text-xl font-bold text-gray-100">Dashboard</h1>
        <p className="text-sm text-muted mt-0.5">System overview and memory status</p>
      </div>

      {/* Stats */}
      <div className="grid grid-cols-2 xl:grid-cols-4 gap-4">
        <StatCard icon={Monitor} label="Connected devices" value={approved.length} color="text-accent" />
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

      {/* Local memory pools */}
      <div>
        <h2 className="text-sm font-semibold text-gray-300 mb-3">Local Memory Pools</h2>
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

      {/* Network cluster memory */}
      {approved.length > 0 && (
        <div>
          <div className="flex items-center justify-between mb-3">
            <h2 className="text-sm font-semibold text-gray-300">Network Cluster</h2>
            {clusterTotalMb > 0 && (
              <span className="text-xs text-muted">
                {fmt(clusterTotalMb)} total across {clusterDevices.length} device{clusterDevices.length !== 1 ? 's' : ''}
              </span>
            )}
          </div>
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
            {approved.map(d => {
              const ready = d.rpc_status === 'ready'
              const usedMb = d.memory_total_mb - d.memory_free_mb
              const pct = d.memory_total_mb > 0 ? Math.round((usedMb / d.memory_total_mb) * 100) : 0
              return (
                <div key={d.id} className="card">
                  <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center gap-2 min-w-0">
                      {ready
                        ? <Wifi size={13} className="text-success flex-shrink-0" />
                        : <WifiOff size={13} className="text-muted flex-shrink-0" />}
                      <span className="text-sm font-medium text-gray-200 truncate">{d.name}</span>
                      <span className="text-xs text-muted font-mono">{d.ip}</span>
                    </div>
                    <span className={clsx(
                      'text-xs px-1.5 py-0.5 rounded flex-shrink-0',
                      ready ? 'bg-success/15 text-success' : 'bg-surface text-muted'
                    )}>
                      {d.rpc_status}
                    </span>
                  </div>
                  {d.memory_total_mb > 0 ? (
                    <div>
                      <div className="h-1.5 bg-surface rounded-full overflow-hidden mb-1">
                        <div
                          className={clsx(
                            'h-full rounded-full transition-all',
                            pct > 85 ? 'bg-danger' : pct > 60 ? 'bg-warning' : 'bg-success'
                          )}
                          style={{ width: `${pct}%` }}
                        />
                      </div>
                      <div className="flex justify-between text-xs text-muted">
                        <span>{fmt(usedMb)} used</span>
                        <span>{fmt(d.memory_total_mb)} total</span>
                      </div>
                    </div>
                  ) : (
                    <p className="text-xs text-muted">
                      Agent not running · port {d.rpc_port} ·{' '}
                      <a href="/agent" className="text-accent hover:underline">Install agent</a>
                    </p>
                  )}
                </div>
              )
            })}
          </div>
        </div>
      )}

      {/* Recent devices table */}
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
