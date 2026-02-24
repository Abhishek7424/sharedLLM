import { useEffect, useRef, useCallback } from 'react'
import type { WsEvent } from '../types'

const WS_URL = (import.meta.env.VITE_API_URL ?? 'http://localhost:8080')
  .replace(/^http/, 'ws') + '/ws'

type Handler = (event: WsEvent) => void

export function useWebSocket(onEvent: Handler) {
  const ws = useRef<WebSocket | null>(null)
  const reconnectTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const handlerRef = useRef<Handler>(onEvent)
  handlerRef.current = onEvent

  const connect = useCallback(() => {
    try {
      const socket = new WebSocket(WS_URL)
      ws.current = socket

      socket.onopen = () => {
        console.log('[WS] connected')
      }

      socket.onmessage = (e) => {
        try {
          const event = JSON.parse(e.data) as WsEvent
          handlerRef.current(event)
        } catch {
          console.warn('[WS] invalid message:', e.data)
        }
      }

      socket.onclose = () => {
        console.log('[WS] disconnected — reconnecting in 3s')
        reconnectTimer.current = setTimeout(connect, 3000)
      }

      socket.onerror = () => {
        socket.close()
      }
    } catch (err) {
      console.warn('[WS] connection error:', err)
      reconnectTimer.current = setTimeout(connect, 3000)
    }
  }, [])

  useEffect(() => {
    connect()
    return () => {
      if (reconnectTimer.current) clearTimeout(reconnectTimer.current)
      ws.current?.close()
    }
  }, [connect])
}
