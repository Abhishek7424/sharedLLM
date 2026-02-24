# sharedLLM

Share your GPU / unified memory with other devices on your local network and run large language models collaboratively — no cloud, no accounts, fully self-hosted.

A host machine (MacBook, Linux workstation, etc.) advertises its GPU or unified memory over mDNS. Other devices on the same WiFi or LAN appear in the dashboard, the host approves them with a role, and they get a slice of memory to run models via Ollama.

---

## Features

- **Auto-discovery** — devices found via mDNS appear in the dashboard instantly as pending
- **Approval flow** — toast notification on new device join; approve with a role or deny
- **Role-based allocation** — admin (16 GB), user (4 GB), guest (1 GB); fully customizable
- **Multi-platform GPU detection** — Apple Silicon unified memory, NVIDIA, AMD, Intel iGPU, system RAM fallback
- **Ollama integration** — list models, pull with live progress, delete, auto-start + watchdog
- **Real-time dashboard** — WebSocket-driven memory bars, device cards, live stats every 3 s
- **SQLite persistence** — devices, roles, allocations, settings survive restarts
- **Single binary** — backend serves the built frontend; one process, one port

---

## Tech stack

| Layer | Technology |
|---|---|
| Backend | Rust, Axum 0.7, sqlx, SQLite |
| Frontend | React 19, TypeScript, Vite, Tailwind CSS |
| Real-time | WebSocket (tokio broadcast channel) |
| Discovery | mDNS (mdns-sd) |
| LLM runtime | Ollama |
| GPU probing | nvidia-smi · rocm-smi · vm_stat · sysfs |

---

## Project structure

```
shared-memory/
├── start.sh              # production: build frontend → run backend
├── dev.sh                # development: backend + Vite dev server in parallel
├── backend/
│   ├── Cargo.toml
│   ├── migrations/       # SQLite migrations (sqlx)
│   └── src/
│       ├── main.rs       # router, app state, startup
│       ├── api/          # HTTP handlers (devices, gpu, models, permissions, settings, ws)
│       ├── db/           # sqlx models + queries
│       ├── memory/       # GPU provider traits (apple, nvidia, amd, intel, system_ram)
│       ├── discovery/    # mDNS advertise + browse
│       ├── ollama/       # Ollama process manager + watchdog
│       ├── permissions/  # device registration + approval logic
│       └── ws/           # WebSocket event types + broadcaster
└── frontend/
    └── src/
        ├── pages/        # Dashboard · Devices · Models · Permissions · Settings
        ├── components/   # DeviceCard · MemoryBar · ApprovalToast · RoleEditor · Sidebar
        ├── hooks/        # useWebSocket · useDevices · useMemory
        ├── lib/api.ts    # typed API client
        └── types/        # shared TypeScript types
```

---

## Prerequisites

| Tool | Version | Install |
|---|---|---|
| Rust + Cargo | stable | https://rustup.rs |
| Node.js | 18+ | https://nodejs.org |
| Ollama | any | https://ollama.com |

---

## Quick start

### Production (single server, port 8080)

```bash
git clone https://github.com/Abhishek7424/sharedLLM.git
cd sharedLLM/shared-memory
./start.sh
```

Open **http://localhost:8080** in your browser.

`start.sh` will:
1. Install frontend npm dependencies if missing
2. Build the React app into `frontend/dist/`
3. Compile and run the Rust backend (serves the frontend statically)

### Development (hot-reload)

```bash
./dev.sh
```

- Frontend dev server: **http://localhost:5173** (Vite HMR)
- Backend API: **http://localhost:8080**

Ctrl+C stops both processes.

---

## API reference

All endpoints are under `http://localhost:8080`.

| Method | Path | Description |
|---|---|---|
| `GET` | `/api/gpu` | Memory stats from all detected providers |
| `GET` | `/api/devices` | List all devices |
| `POST` | `/api/devices` | Manually add a device `{name, ip}` |
| `GET` | `/api/devices/:id` | Get single device |
| `DELETE` | `/api/devices/:id` | Remove device |
| `POST` | `/api/devices/:id/approve` | Approve with role `{role_id}` |
| `POST` | `/api/devices/:id/deny` | Deny device |
| `PATCH` | `/api/devices/:id/memory` | Set allocation `{memory_mb}` |
| `GET` | `/api/permissions/roles` | List roles |
| `POST` | `/api/permissions/roles` | Create role |
| `PUT` | `/api/permissions/roles/:id` | Update role |
| `DELETE` | `/api/permissions/roles/:id` | Delete role (not built-ins) |
| `GET` | `/api/models` | List Ollama models |
| `POST` | `/api/models/pull` | Pull model (streams progress) `{name}` |
| `DELETE` | `/api/models/:name` | Delete model |
| `GET` | `/api/ollama/status` | Ollama running status |
| `GET` | `/api/settings` | All settings |
| `PUT` | `/api/settings/:key` | Update a setting `{value}` |
| `GET` | `/ws` | WebSocket — real-time events |

### WebSocket events

Events are JSON objects with a `type` field:

```jsonc
{ "type": "memory_stats", "snapshots": [...] }          // every 3 s
{ "type": "device_pending_approval", "device_id": "…" } // new device joined
{ "type": "device_approved", "device_id": "…" }
{ "type": "device_denied", "device_id": "…" }
{ "type": "device_discovered", "ip": "…", "name": "…" } // mDNS discovery
{ "type": "ollama_status", "running": true, "host": "…" }
```

---

## Configuration (settings API)

| Key | Default | Description |
|---|---|---|
| `api_port` | `8080` | Server port |
| `ollama_host` | `http://127.0.0.1:11434` | Ollama base URL |
| `auto_start_ollama` | `true` | Launch Ollama on startup |
| `mdns_enabled` | `true` | Discover other devices on LAN |
| `trust_local_network` | `false` | Auto-approve LAN devices |
| `default_role` | `role-guest` | Role assigned to auto-approved devices |

Settings are persisted in SQLite and can be updated live via `PUT /api/settings/:key`.

---

## Default roles

| Role | Max memory | Can pull models | Trust level |
|---|---|---|---|
| admin | 16 384 MB | yes | 3 |
| user | 4 096 MB | yes | 2 |
| guest | 1 024 MB | no | 1 |

Custom roles can be created from the Permissions page.

---

## GPU support

| Platform | Detection method |
|---|---|
| Apple Silicon | `sysctl hw.memsize` + `vm_stat` |
| NVIDIA | `nvidia-smi` |
| AMD | `rocm-smi` + sysfs |
| Intel iGPU | sysfs (`/sys/class/drm`) |
| Fallback | system RAM via `sysinfo` |

On Apple Silicon the unified memory pool serves as both GPU and system RAM. The system RAM provider is automatically skipped to avoid double-counting.

---

## License

MIT
