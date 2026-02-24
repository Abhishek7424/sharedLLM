// ─── Device ──────────────────────────────────────────────────────────────────

export type DeviceStatus = 'pending' | 'approved' | 'denied' | 'suspended' | 'offline'
export type DiscoveryMethod = 'mdns' | 'manual'

export interface Device {
  id: string
  name: string
  ip: string
  mac?: string
  hostname?: string
  platform?: string
  role_id?: string
  status: DeviceStatus
  discovery_method: DiscoveryMethod
  allocated_memory_mb: number
  last_seen?: string
  first_seen: string
  created_at: string
}

// ─── Role ─────────────────────────────────────────────────────────────────────

export interface Role {
  id: string
  name: string
  max_memory_mb: number
  can_pull_models: number // 0 | 1
  trust_level: number
  created_at: string
}

// ─── Memory / GPU ─────────────────────────────────────────────────────────────

export type GpuKind = 'nvidia' | 'amd' | 'apple_silicon' | 'intel' | 'system_ram'

export interface MemorySnapshot {
  provider_id: string
  name: string
  kind: GpuKind
  total_mb: number
  used_mb: number
  free_mb: number
  allocated_mb: number
}

// ─── Ollama ───────────────────────────────────────────────────────────────────

export interface OllamaModel {
  name: string
  size: number
  digest: string
  modified_at: string
}

// ─── WebSocket Events ─────────────────────────────────────────────────────────

export type WsEventType =
  | 'device_discovered'
  | 'device_pending_approval'
  | 'device_approved'
  | 'device_denied'
  | 'device_offline'
  | 'memory_allocated'
  | 'memory_stats'
  | 'ollama_status'
  | 'error'

export interface WsEventDeviceDiscovered {
  type: 'device_discovered'
  ip: string
  name: string
  hostname: string
  method: string
}

export interface WsEventPendingApproval {
  type: 'device_pending_approval'
  device_id: string
  name: string
  ip: string
  discovery_method: string
}

export interface WsEventApproved {
  type: 'device_approved'
  device_id: string
  name: string
  ip: string
}

export interface WsEventDenied {
  type: 'device_denied'
  device_id: string
}

export interface WsEventOffline {
  type: 'device_offline'
  name: string
}

export interface WsEventMemoryAllocated {
  type: 'memory_allocated'
  device_id: string
  memory_mb: number
}

export interface WsEventMemoryStats {
  type: 'memory_stats'
  snapshots: MemorySnapshot[]
}

export interface WsEventOllamaStatus {
  type: 'ollama_status'
  running: boolean
  host: string
}

export interface WsEventError {
  type: 'error'
  message: string
}

export type WsEvent =
  | WsEventDeviceDiscovered
  | WsEventPendingApproval
  | WsEventApproved
  | WsEventDenied
  | WsEventOffline
  | WsEventMemoryAllocated
  | WsEventMemoryStats
  | WsEventOllamaStatus
  | WsEventError

// ─── Settings ─────────────────────────────────────────────────────────────────

export type Settings = Record<string, string>
