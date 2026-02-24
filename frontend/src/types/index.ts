// ─── Device ──────────────────────────────────────────────────────────────────

export type DeviceStatus = 'pending' | 'approved' | 'denied' | 'suspended' | 'offline'
export type DiscoveryMethod = 'mdns' | 'manual'
export type RpcStatus = 'offline' | 'connecting' | 'ready' | 'error'

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
  // Distributed inference fields
  rpc_port: number
  rpc_status: RpcStatus
  memory_total_mb: number
  memory_free_mb: number
}

// ─── Role ─────────────────────────────────────────────────────────────────────

export interface Role {
  id: string
  name: string
  max_memory_mb: number
  can_pull_models: boolean
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

// ─── Distributed inference ────────────────────────────────────────────────────

export interface InferenceSessionInfo {
  id: string
  model_path: string
  status: string // starting | running | stopped | error
  rpc_devices: string[] // "ip:port" strings
  started_at: string
}

export interface LlamaCppStatus {
  rpc_server_running: boolean
  inference_running: boolean
  rpc_server_bin: boolean
  inference_server_bin: boolean
  rpc_port: number
  inference_port: number
  current_session?: InferenceSessionInfo
}

export interface ClusterDeviceStatus {
  id: string
  name: string
  ip: string
  rpc_port: number
  rpc_status: RpcStatus
  memory_total_mb: number
  memory_free_mb: number
}

export interface ClusterStatus {
  devices: ClusterDeviceStatus[]
  llama_cpp: LlamaCppStatus
  current_session?: InferenceSessionInfo
}

export interface AgentInfo {
  host_ip: string
  dashboard_port: string
  rpc_port: number
  install_commands: {
    linux: string
    macos: string
    windows: string
  }
  rpc_server_bin_available: boolean
}

// ─── Chat (OpenAI-compatible) ─────────────────────────────────────────────────

export interface ChatMessage {
  role: 'user' | 'assistant' | 'system'
  content: string
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
  | 'rpc_server_ready'
  | 'rpc_server_offline'
  | 'rpc_device_ready'
  | 'rpc_device_offline'
  | 'inference_started'
  | 'inference_stopped'
  | 'layer_assignment'

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

export interface WsEventRpcServerReady {
  type: 'rpc_server_ready'
  port: number
}

export interface WsEventRpcServerOffline {
  type: 'rpc_server_offline'
}

export interface WsEventRpcDeviceReady {
  type: 'rpc_device_ready'
  device_id: string
  memory_total_mb: number
  memory_free_mb: number
}

export interface WsEventRpcDeviceOffline {
  type: 'rpc_device_offline'
  device_id: string
}

export interface WsEventInferenceStarted {
  type: 'inference_started'
  session_id: string
  model: string
  devices: string[]
}

export interface WsEventInferenceStopped {
  type: 'inference_stopped'
  session_id: string
}

export interface LayerAssignment {
  device_id: string
  layers: string
}

export interface WsEventLayerAssignment {
  type: 'layer_assignment'
  assignments: LayerAssignment[]
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
  | WsEventRpcServerReady
  | WsEventRpcServerOffline
  | WsEventRpcDeviceReady
  | WsEventRpcDeviceOffline
  | WsEventInferenceStarted
  | WsEventInferenceStopped
  | WsEventLayerAssignment

// ─── Settings ─────────────────────────────────────────────────────────────────

export type Settings = Record<string, string>
