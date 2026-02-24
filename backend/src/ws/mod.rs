use serde::{Deserialize, Serialize};

/// All WebSocket events sent to connected browser clients
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEvent {
    /// A new device was discovered via mDNS
    DeviceDiscovered {
        ip: String,
        name: String,
        hostname: String,
        method: String,
    },
    /// A device is waiting for manual approval
    DevicePendingApproval {
        device_id: String,
        name: String,
        ip: String,
        discovery_method: String,
    },
    /// A device was approved
    DeviceApproved {
        device_id: String,
        name: String,
        ip: String,
    },
    /// A device was denied
    DeviceDenied { device_id: String },
    /// A device went offline (mDNS removal)
    DeviceOffline { name: String },
    /// Memory was allocated to a device
    MemoryAllocated { device_id: String, memory_mb: i64 },
    /// Periodic GPU/memory stats update
    MemoryStats {
        snapshots: Vec<crate::memory::MemorySnapshot>,
    },
    /// Ollama status changed
    OllamaStatus { running: bool, host: String },
    /// Generic error notification
    Error { message: String },
}
