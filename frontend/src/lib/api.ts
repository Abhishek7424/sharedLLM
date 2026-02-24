const API_BASE = import.meta.env.VITE_API_URL ?? 'http://localhost:8080'
const WS_BASE = API_BASE.replace(/^http/, 'ws')

/** Throw an error containing the response body if the HTTP status is not 2xx. */
async function checkOk(r: Response): Promise<Response> {
  if (!r.ok) {
    let msg = `HTTP ${r.status}`
    try {
      const body = await r.json()
      msg = body?.error ?? JSON.stringify(body) ?? msg
    } catch {}
    throw new Error(msg)
  }
  return r
}

export const api = {
  base: API_BASE,
  ws: `${WS_BASE}/ws`,

  // Devices
  devices: () => fetch(`${API_BASE}/api/devices`).then(checkOk).then(r => r.json()),
  getDevice: (id: string) => fetch(`${API_BASE}/api/devices/${id}`).then(checkOk).then(r => r.json()),
  addDevice: (body: { name: string; ip: string; mac?: string }) =>
    fetch(`${API_BASE}/api/devices`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    }).then(checkOk).then(r => r.json()),
  approveDevice: (id: string, role_id?: string) =>
    fetch(`${API_BASE}/api/devices/${id}/approve`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ role_id }),
    }).then(checkOk).then(r => r.json()),
  denyDevice: (id: string) =>
    fetch(`${API_BASE}/api/devices/${id}/deny`, { method: 'POST' }).then(checkOk).then(r => r.json()),
  allocateMemory: (id: string, memory_mb: number) =>
    fetch(`${API_BASE}/api/devices/${id}/memory`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ memory_mb }),
    }).then(checkOk).then(r => r.json()),
  deleteDevice: (id: string) =>
    fetch(`${API_BASE}/api/devices/${id}`, { method: 'DELETE' }).then(checkOk).then(r => r.json()),

  // GPU
  gpuStats: () => fetch(`${API_BASE}/api/gpu`).then(checkOk).then(r => r.json()),

  // Models
  models: () => fetch(`${API_BASE}/api/models`).then(checkOk).then(r => r.json()),
  /**
   * Pull a model — returns a ReadableStream of NDJSON progress lines.
   * Caller is responsible for reading the stream and parsing each line.
   */
  pullModel: (name: string) =>
    fetch(`${API_BASE}/api/models/pull`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name }),
    }),
  deleteModel: (name: string) =>
    fetch(`${API_BASE}/api/models/${encodeURIComponent(name)}`, { method: 'DELETE' }).then(checkOk).then(r => r.json()),
  ollamaStatus: () => fetch(`${API_BASE}/api/ollama/status`).then(checkOk).then(r => r.json()),

  // Permissions
  roles: () => fetch(`${API_BASE}/api/permissions/roles`).then(checkOk).then(r => r.json()),
  createRole: (body: { name: string; max_memory_mb: number; can_pull_models: boolean; trust_level: number }) =>
    fetch(`${API_BASE}/api/permissions/roles`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    }).then(checkOk).then(r => r.json()),
  updateRole: (id: string, body: { name: string; max_memory_mb: number; can_pull_models: boolean; trust_level: number }) =>
    fetch(`${API_BASE}/api/permissions/roles/${id}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    }).then(checkOk).then(r => r.json()),
  deleteRole: (id: string) =>
    fetch(`${API_BASE}/api/permissions/roles/${id}`, { method: 'DELETE' }).then(checkOk).then(r => r.json()),

  // Settings
  settings: () => fetch(`${API_BASE}/api/settings`).then(checkOk).then(r => r.json()),
  updateSetting: (key: string, value: string) =>
    fetch(`${API_BASE}/api/settings/${key}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ value }),
    }).then(checkOk).then(r => r.json()),

  // Cluster / Distributed inference
  clusterStatus: () =>
    fetch(`${API_BASE}/api/cluster/status`).then(checkOk).then(r => r.json()),
  inferenceStatus: () =>
    fetch(`${API_BASE}/api/cluster/inference/status`).then(checkOk).then(r => r.json()),
  startInference: (model_path: string, device_ids: string[]) =>
    fetch(`${API_BASE}/api/cluster/inference/start`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ model_path, device_ids }),
    }).then(checkOk).then(r => r.json()),
  stopInference: () =>
    fetch(`${API_BASE}/api/cluster/inference/stop`, { method: 'POST' }).then(checkOk).then(r => r.json()),
  startRpcServer: () =>
    fetch(`${API_BASE}/api/cluster/rpc/start`, { method: 'POST' }).then(checkOk).then(r => r.json()),
  stopRpcServer: () =>
    fetch(`${API_BASE}/api/cluster/rpc/stop`, { method: 'POST' }).then(checkOk).then(r => r.json()),

  // Agent install info
  agentInfo: () =>
    fetch(`${API_BASE}/agent/info`).then(checkOk).then(r => r.json()),
  agentInstallUrl: (os: 'linux' | 'macos' | 'windows') =>
    `${API_BASE}/agent/install?os=${os}`,

  // OpenAI-compatible chat (proxied to llama-server)
  chatCompletions: (messages: Array<{ role: string; content: string }>, model = 'local', stream = false) =>
    fetch(`${API_BASE}/v1/chat/completions`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ model, messages, stream }),
    }),
}
