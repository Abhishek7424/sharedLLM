import { NavLink } from 'react-router-dom'
import { LayoutDashboard, Monitor, Shield, Package, Settings, Cpu, Terminal, Download, MessageSquare } from 'lucide-react'
import { clsx } from 'clsx'

const links = [
  { to: '/', icon: LayoutDashboard, label: 'Dashboard' },
  { to: '/devices', icon: Monitor, label: 'Devices' },
  { to: '/permissions', icon: Shield, label: 'Permissions' },
  { to: '/models', icon: Package, label: 'Models' },
  { to: '/inference', icon: Terminal, label: 'Inference' },
  { to: '/chat', icon: MessageSquare, label: 'Chat' },
  { to: '/agent', icon: Download, label: 'Agent' },
  { to: '/settings', icon: Settings, label: 'Settings' },
]

export function Sidebar() {
  return (
    <aside className="w-52 flex-shrink-0 bg-panel border-r border-border flex flex-col h-screen sticky top-0">
      {/* Logo */}
      <div className="p-5 border-b border-border">
        <div className="flex items-center gap-2.5">
          <div className="w-8 h-8 rounded-lg bg-accent flex items-center justify-center">
            <Cpu size={18} className="text-white" />
          </div>
          <div>
            <p className="text-sm font-bold text-gray-100 leading-none">SharedMem</p>
            <p className="text-xs text-muted mt-0.5">Network v0.1</p>
          </div>
        </div>
      </div>

      {/* Nav */}
      <nav className="flex-1 p-3 space-y-0.5">
        {links.map(({ to, icon: Icon, label }) => (
          <NavLink
            key={to}
            to={to}
            end={to === '/'}
            className={({ isActive }) =>
              clsx(
                'flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm transition-colors',
                isActive
                  ? 'bg-accent/15 text-accent font-medium'
                  : 'text-muted hover:text-gray-300 hover:bg-white/5'
              )
            }
          >
            <Icon size={16} />
            {label}
          </NavLink>
        ))}
      </nav>
    </aside>
  )
}
