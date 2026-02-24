import { clsx } from 'clsx'
import type { MemorySnapshot, GpuKind } from '../types'

interface MemoryBarProps {
  snapshot: MemorySnapshot
  compact?: boolean
}

const kindLabel: Record<GpuKind, string> = {
  nvidia: 'NVIDIA',
  amd: 'AMD',
  apple_silicon: 'Apple Silicon',
  intel: 'Intel iGPU',
  system_ram: 'System RAM',
}

const kindColor: Record<GpuKind, string> = {
  nvidia: '#76b900',
  amd: '#e2231a',
  apple_silicon: '#a2aaad',
  intel: '#0071c5',
  system_ram: '#6366f1',
}

function fmt(mb: number): string {
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`
  return `${mb} MB`
}

export function MemoryBar({ snapshot, compact = false }: MemoryBarProps) {
  const usedPct = snapshot.total_mb > 0
    ? Math.round((snapshot.used_mb / snapshot.total_mb) * 100)
    : 0
  const allocPct = snapshot.total_mb > 0
    ? Math.round((snapshot.allocated_mb / snapshot.total_mb) * 100)
    : 0

  const color = kindColor[snapshot.kind]

  return (
    <div className={clsx('space-y-2', compact ? 'py-1' : 'py-2')}>
      <div className="flex items-center justify-between text-sm">
        <span className="text-gray-200 font-medium">{snapshot.name}</span>
        <span className="text-muted text-xs">{kindLabel[snapshot.kind]}</span>
      </div>

      {/* Used bar */}
      <div className="space-y-1">
        <div className="flex justify-between text-xs text-muted">
          <span>Used</span>
          <span>{fmt(snapshot.used_mb)} / {fmt(snapshot.total_mb)} ({usedPct}%)</span>
        </div>
        <div className="h-2 bg-surface rounded-full overflow-hidden">
          <div
            className="h-full rounded-full transition-all duration-500"
            style={{ width: `${usedPct}%`, backgroundColor: color, opacity: 0.7 }}
          />
        </div>
      </div>

      {/* Allocated bar */}
      <div className="space-y-1">
        <div className="flex justify-between text-xs text-muted">
          <span>Allocated to devices</span>
          <span>{fmt(snapshot.allocated_mb)} ({allocPct}%)</span>
        </div>
        <div className="h-2 bg-surface rounded-full overflow-hidden">
          <div
            className="h-full rounded-full transition-all duration-500"
            style={{ width: `${allocPct}%`, backgroundColor: color }}
          />
        </div>
      </div>

      <div className="flex justify-between text-xs">
        <span className="text-muted">Free</span>
        <span className="text-success">{fmt(snapshot.free_mb)}</span>
      </div>
    </div>
  )
}
