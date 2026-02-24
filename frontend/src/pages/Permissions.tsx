import { api } from '../lib/api'
import type { Role } from '../types'
import { RoleEditor } from '../components/RoleEditor'
import { Shield, ToggleLeft, ToggleRight } from 'lucide-react'

interface PermissionsPageProps {
  roles: Role[]
  onRefresh: () => void
  trustAll: boolean
  onTrustAllChange: (v: boolean) => void
}

export function PermissionsPage({ roles, onRefresh, trustAll, onTrustAllChange }: PermissionsPageProps) {
  const toggle = async () => {
    const next = !trustAll
    await api.updateSetting('trust_local_network', String(next))
    onTrustAllChange(next)
  }

  return (
    <div className="p-6 space-y-6">
      <div>
        <h1 className="text-xl font-bold text-gray-100">Permissions</h1>
        <p className="text-sm text-muted mt-0.5">Manage roles and network trust settings</p>
      </div>

      {/* Trust all toggle */}
      <div className="card flex items-center justify-between gap-6">
        <div className="flex items-start gap-3">
          <div className={`w-9 h-9 rounded-lg flex items-center justify-center flex-shrink-0 ${trustAll ? 'bg-success/15 text-success' : 'bg-muted/15 text-muted'}`}>
            <Shield size={18} />
          </div>
          <div>
            <p className="text-sm font-semibold text-gray-100">Trust local network</p>
            <p className="text-xs text-muted mt-0.5">
              When enabled, any device discovered on the LAN is automatically approved with the default role.
              Disable to manually approve each device.
            </p>
          </div>
        </div>
        <button onClick={toggle} className={`flex-shrink-0 transition-colors ${trustAll ? 'text-success' : 'text-muted'}`}>
          {trustAll ? <ToggleRight size={36} /> : <ToggleLeft size={36} />}
        </button>
      </div>

      {/* Role editor */}
      <div>
        <h2 className="text-sm font-semibold text-gray-300 mb-3">Roles</h2>
        <RoleEditor roles={roles} onRefresh={onRefresh} />
      </div>
    </div>
  )
}
