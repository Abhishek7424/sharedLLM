import { clsx } from 'clsx'
import { Monitor, Wifi, WifiOff, Clock, HardDrive, Check, X, MoreHorizontal } from 'lucide-react'
import type { Device, DeviceStatus, Role } from '../types'
import { useState } from 'react'

interface DeviceCardProps {
  device: Device
  roles: Role[]
  onApprove: (id: string, roleId?: string) => void
  onDeny: (id: string) => void
  onAllocate: (id: string, mb: number) => void
  onRemove: (id: string) => void
}

function StatusBadge({ status }: { status: DeviceStatus }) {
  const cls: Record<DeviceStatus, string> = {
    pending: 'badge-pending',
    approved: 'badge-approved',
    denied: 'badge-denied',
    offline: 'badge-offline',
    suspended: 'badge-suspended',
  }
  const dot: Record<DeviceStatus, string> = {
    pending: 'bg-warning',
    approved: 'bg-success',
    denied: 'bg-danger',
    offline: 'bg-muted',
    suspended: 'bg-orange-400',
  }
  return (
    <span className={cls[status]}>
      <span className={clsx('w-1.5 h-1.5 rounded-full', dot[status])} />
      {status}
    </span>
  )
}

function fmt(mb: number) {
  return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${mb} MB`
}

function timeAgo(iso?: string) {
  if (!iso) return 'never'
  const diff = (Date.now() - new Date(iso).getTime()) / 1000
  if (diff < 60) return 'just now'
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
  return `${Math.floor(diff / 86400)}d ago`
}

export function DeviceCard({ device, roles, onApprove, onDeny, onAllocate, onRemove }: DeviceCardProps) {
  const [selectedRole, setSelectedRole] = useState(roles[0]?.id ?? '')
  const [memInput, setMemInput] = useState(String(device.allocated_memory_mb || 512))
  const [showActions, setShowActions] = useState(false)

  return (
    <div className={clsx(
      'card relative transition-all duration-200',
      device.status === 'pending' && 'border-warning/50 shadow-[0_0_0_1px_rgba(245,158,11,0.3)]'
    )}>
      {/* Header */}
      <div className="flex items-start justify-between gap-3 mb-3">
        <div className="flex items-center gap-3">
          <div className={clsx(
            'w-9 h-9 rounded-lg flex items-center justify-center',
            device.status === 'approved' ? 'bg-accent/20 text-accent' : 'bg-surface text-muted'
          )}>
            <Monitor size={18} />
          </div>
          <div>
            <h3 className="font-semibold text-sm text-gray-100">{device.name}</h3>
            <p className="text-xs text-muted">{device.ip}</p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <StatusBadge status={device.status as DeviceStatus} />
          <button
            onClick={() => setShowActions(v => !v)}
            className="text-muted hover:text-gray-300 transition-colors p-1"
          >
            <MoreHorizontal size={16} />
          </button>
        </div>
      </div>

      {/* Meta */}
      <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs text-muted mb-3">
        <span className="flex items-center gap-1">
          {device.discovery_method === 'mdns' ? <Wifi size={12} /> : <WifiOff size={12} />}
          {device.discovery_method}
        </span>
        <span className="flex items-center gap-1">
          <Clock size={12} />
          {timeAgo(device.last_seen ?? undefined)}
        </span>
        {device.mac && <span>MAC: {device.mac}</span>}
        {device.hostname && <span className="truncate">{device.hostname}</span>}
        {device.allocated_memory_mb > 0 && (
          <span className="flex items-center gap-1 col-span-2 text-accent">
            <HardDrive size={12} />
            {fmt(device.allocated_memory_mb)} allocated
          </span>
        )}
      </div>

      {/* Approval actions (pending only) */}
      {device.status === 'pending' && (
        <div className="border-t border-border pt-3 space-y-2">
          <p className="text-xs text-warning font-medium">Awaiting permission to join</p>
          <div className="flex gap-2">
            <select
              className="input text-xs py-1 flex-1"
              value={selectedRole}
              onChange={e => setSelectedRole(e.target.value)}
            >
              {roles.map(r => (
                <option key={r.id} value={r.id}>{r.name} (max {fmt(r.max_memory_mb)})</option>
              ))}
            </select>
          </div>
          <div className="flex gap-2">
            <button
              onClick={() => onApprove(device.id, selectedRole)}
              className="btn-success flex-1 text-xs py-1.5"
            >
              <Check size={14} /> Approve
            </button>
            <button
              onClick={() => onDeny(device.id)}
              className="btn-danger flex-1 text-xs py-1.5"
            >
              <X size={14} /> Deny
            </button>
          </div>
        </div>
      )}

      {/* Memory allocation (approved only) */}
      {device.status === 'approved' && showActions && (
        <div className="border-t border-border pt-3 space-y-2">
          <p className="text-xs text-muted font-medium">Allocate memory (MB)</p>
          <div className="flex gap-2">
            <input
              type="number"
              className="input text-xs py-1 flex-1"
              value={memInput}
              min={0}
              step={256}
              onChange={e => setMemInput(e.target.value)}
            />
            <button
              onClick={() => onAllocate(device.id, Number(memInput))}
              className="btn-primary text-xs py-1.5 px-3"
            >
              Set
            </button>
            <button
              onClick={() => onRemove(device.id)}
              className="btn-danger text-xs py-1.5 px-3"
            >
              <X size={14} />
            </button>
          </div>
        </div>
      )}

      {/* Denied / other — show remove */}
      {(device.status === 'denied' || device.status === 'offline') && showActions && (
        <div className="border-t border-border pt-3">
          <button onClick={() => onRemove(device.id)} className="btn-ghost text-xs w-full">
            Remove from list
          </button>
        </div>
      )}
    </div>
  )
}
