import { useState, useCallback, useEffect } from 'react'
import { BrowserRouter, Routes, Route } from 'react-router-dom'

import { Sidebar } from './components/Sidebar'
import { ApprovalToast } from './components/ApprovalToast'
import type { ApprovalRequest } from './components/ApprovalToast'

import { Dashboard } from './pages/Dashboard'
import { DevicesPage } from './pages/Devices'
import { PermissionsPage } from './pages/Permissions'
import { ModelsPage } from './pages/Models'
import { SettingsPage } from './pages/Settings'
import { InferencePage } from './pages/Inference'
import { AgentPage } from './pages/Agent'

import { useWebSocket } from './hooks/useWebSocket'
import { useDevices } from './hooks/useDevices'
import { useMemory } from './hooks/useMemory'

import type { WsEvent, Role, OllamaModel, Settings } from './types'
import { api } from './lib/api'

export default function App() {
  const { devices, loading: devLoading, refresh: refreshDevices, approve, deny, allocate, remove, add } = useDevices()
  const { snapshots, updateFromWs } = useMemory()

  const [roles, setRoles] = useState<Role[]>([])
  const fetchRoles = useCallback(async () => {
    try { const d = await api.roles(); setRoles(d.roles ?? []) } catch {}
  }, [])
  useEffect(() => { fetchRoles() }, [fetchRoles])

  const [ollamaRunning, setOllamaRunning] = useState(false)
  const [ollamaHost, setOllamaHost] = useState('http://localhost:11434')
  const [models, setModels] = useState<OllamaModel[]>([])

  useEffect(() => {
    api.ollamaStatus().then(d => {
      setOllamaRunning(d.running ?? false)
      setOllamaHost(d.host ?? 'http://localhost:11434')
    }).catch(() => {})
  }, [])

  useEffect(() => {
    if (ollamaRunning) {
      api.models().then(d => setModels(d.models ?? [])).catch(() => {})
    }
  }, [ollamaRunning])

  const [settings, setSettings] = useState<Settings>({})
  const [trustAll, setTrustAll] = useState(false)
  useEffect(() => {
    api.settings().then(s => {
      setSettings(s)
      setTrustAll(s.trust_local_network === 'true')
    }).catch(() => {})
  }, [])

  const [approvalRequests, setApprovalRequests] = useState<ApprovalRequest[]>([])

  // Distributed inference state (tracked for cross-page awareness via WS)
  const [, setRpcRunning] = useState(false)
  const [, setInferenceRunning] = useState(false)

  const handleWsEvent = useCallback((event: WsEvent) => {
    switch (event.type) {
      case 'device_pending_approval':
        setApprovalRequests(prev => {
          if (prev.some(r => r.device_id === event.device_id)) return prev
          return [...prev, {
            device_id: event.device_id,
            name: event.name,
            ip: event.ip,
            discovery_method: event.discovery_method,
            timestamp: Date.now(),
          }]
        })
        refreshDevices()
        break
      case 'device_approved':
      case 'device_denied':
        setApprovalRequests(prev => prev.filter(r => r.device_id !== event.device_id))
        refreshDevices()
        break
      case 'device_discovered':
      case 'device_offline':
      case 'memory_allocated':
        refreshDevices()
        break
      case 'memory_stats':
        updateFromWs(event.snapshots)
        break
      case 'ollama_status':
        setOllamaRunning(event.running)
        setOllamaHost(event.host)
        break
      case 'rpc_server_ready':
        setRpcRunning(true)
        break
      case 'rpc_server_offline':
        setRpcRunning(false)
        break
      case 'rpc_device_ready':
      case 'rpc_device_offline':
        refreshDevices()
        break
      case 'inference_started':
        setInferenceRunning(true)
        break
      case 'inference_stopped':
        setInferenceRunning(false)
        break
    }
  }, [refreshDevices, updateFromWs])

  useWebSocket(handleWsEvent)

  const handleToastApprove = useCallback(async (id: string) => {
    // Default to the 'guest' role for quick-approvals from the toast notification.
    // Fall back to first role if no guest role exists.
    const guestRole = roles.find(r => r.name.toLowerCase() === 'guest') ?? roles[0]
    await approve(id, guestRole?.id)
    setApprovalRequests(prev => prev.filter(r => r.device_id !== id))
  }, [approve, roles])

  const handleToastDeny = useCallback(async (id: string) => {
    await deny(id)
    setApprovalRequests(prev => prev.filter(r => r.device_id !== id))
  }, [deny])

  return (
    <BrowserRouter>
      <div className="flex min-h-screen bg-surface">
        <Sidebar ollamaRunning={ollamaRunning} />
        <main className="flex-1 overflow-auto">
          <Routes>
            <Route path="/" element={
              <Dashboard devices={devices} snapshots={snapshots} ollamaRunning={ollamaRunning}
                ollamaHost={ollamaHost} models={models} />
            } />
            <Route path="/devices" element={
              <DevicesPage devices={devices} roles={roles} loading={devLoading}
                onRefresh={refreshDevices} onApprove={approve} onDeny={deny}
                onAllocate={allocate} onRemove={remove} onAdd={add} />
            } />
            <Route path="/permissions" element={
              <PermissionsPage roles={roles} onRefresh={fetchRoles}
                trustAll={trustAll} onTrustAllChange={setTrustAll} />
            } />
            <Route path="/models" element={
              <ModelsPage ollamaRunning={ollamaRunning} ollamaHost={ollamaHost} />
            } />
            <Route path="/inference" element={<InferencePage />} />
            <Route path="/agent" element={<AgentPage />} />
            <Route path="/settings" element={
              <SettingsPage settings={settings} onSettingsChange={setSettings} />
            } />
          </Routes>
        </main>
        <ApprovalToast requests={approvalRequests} onApprove={handleToastApprove} onDeny={handleToastDeny} />
      </div>
    </BrowserRouter>
  )
}
