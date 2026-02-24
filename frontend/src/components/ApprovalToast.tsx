import { Bell, Check, X, Wifi } from 'lucide-react'

export interface ApprovalRequest {
  device_id: string
  name: string
  ip: string
  discovery_method: string
  timestamp: number
}

interface ApprovalToastProps {
  requests: ApprovalRequest[]
  onApprove: (id: string) => void
  onDeny: (id: string) => void
}

export function ApprovalToast({ requests, onApprove, onDeny }: ApprovalToastProps) {
  if (requests.length === 0) return null

  return (
    <div className="fixed top-4 right-4 z-50 space-y-3 max-w-sm w-full">
      {requests.map(req => (
        <div
          key={req.device_id}
          className="bg-panel border border-warning/50 rounded-xl p-4 shadow-2xl
                     animate-in slide-in-from-right-4 duration-300"
        >
          {/* Header */}
          <div className="flex items-start gap-3 mb-3">
            <div className="w-8 h-8 rounded-lg bg-warning/15 text-warning flex items-center justify-center flex-shrink-0">
              <Bell size={16} />
            </div>
            <div className="flex-1 min-w-0">
              <p className="text-sm font-semibold text-gray-100">Device wants to join</p>
              <p className="text-xs text-warning truncate font-medium">{req.name}</p>
            </div>
          </div>

          {/* Details */}
          <div className="bg-surface rounded-lg px-3 py-2 mb-3 space-y-1">
            <div className="flex items-center gap-2 text-xs text-muted">
              <Wifi size={11} />
              <span className="text-gray-300">{req.ip}</span>
              <span className="ml-auto text-xs">{req.discovery_method}</span>
            </div>
          </div>

          {/* Actions */}
          <div className="flex gap-2">
            <button
              onClick={() => onApprove(req.device_id)}
              className="btn-success flex-1 text-xs py-1.5"
            >
              <Check size={13} /> Allow
            </button>
            <button
              onClick={() => onDeny(req.device_id)}
              className="btn-danger flex-1 text-xs py-1.5"
            >
              <X size={13} /> Block
            </button>
          </div>
        </div>
      ))}
    </div>
  )
}
