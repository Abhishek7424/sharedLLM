import { useState, useEffect, useCallback } from 'react'
import type { Device } from '../types'
import { api } from '../lib/api'

export function useDevices() {
  const [devices, setDevices] = useState<Device[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const fetch = useCallback(async () => {
    try {
      const data = await api.devices()
      setDevices(data.devices ?? [])
      setError(null)
    } catch (e) {
      setError('Failed to fetch devices')
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => { fetch() }, [fetch])

  const approve = useCallback(async (id: string, role_id?: string) => {
    await api.approveDevice(id, role_id)
    await fetch()
  }, [fetch])

  const deny = useCallback(async (id: string) => {
    await api.denyDevice(id)
    await fetch()
  }, [fetch])

  const allocate = useCallback(async (id: string, memory_mb: number) => {
    await api.allocateMemory(id, memory_mb)
    await fetch()
  }, [fetch])

  const remove = useCallback(async (id: string) => {
    await api.deleteDevice(id)
    await fetch()
  }, [fetch])

  const add = useCallback(async (name: string, ip: string, mac?: string) => {
    await api.addDevice({ name, ip, mac })
    await fetch()
  }, [fetch])

  return { devices, loading, error, refresh: fetch, approve, deny, allocate, remove, add }
}
