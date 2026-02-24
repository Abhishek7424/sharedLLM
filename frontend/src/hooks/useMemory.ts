import { useState, useEffect, useCallback } from 'react'
import type { MemorySnapshot } from '../types'
import { api } from '../lib/api'

export function useMemory() {
  const [snapshots, setSnapshots] = useState<MemorySnapshot[]>([])
  const [loading, setLoading] = useState(true)

  const fetch = useCallback(async () => {
    try {
      const data = await api.gpuStats()
      setSnapshots(data.providers ?? [])
    } catch {
      // ignore — updates also come via WS
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => { fetch() }, [fetch])

  const updateFromWs = useCallback((s: MemorySnapshot[]) => {
    setSnapshots(s)
  }, [])

  return { snapshots, loading, refresh: fetch, updateFromWs }
}
