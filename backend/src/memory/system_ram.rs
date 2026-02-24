use super::{GpuKind, MemoryProvider};
use sysinfo::System;

/// System RAM provider — always available as fallback
pub struct SystemRamProvider;

impl SystemRamProvider {
    pub fn new() -> Self {
        SystemRamProvider
    }
}

impl MemoryProvider for SystemRamProvider {
    fn id(&self) -> &str {
        "system_ram"
    }
    fn name(&self) -> &str {
        "System RAM"
    }
    fn kind(&self) -> GpuKind {
        GpuKind::SystemRam
    }

    fn snapshot(&self) -> Option<(u64, u64, u64)> {
        let mut sys = System::new();
        sys.refresh_memory();

        let total_mb = sys.total_memory() / (1024 * 1024);
        let used_mb = sys.used_memory() / (1024 * 1024);
        let free_mb = total_mb.saturating_sub(used_mb);

        Some((total_mb, used_mb, free_mb))
    }
}
