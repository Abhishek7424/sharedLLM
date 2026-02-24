use super::{GpuKind, MemoryProvider};

/// NVIDIA GPU via nvidia-smi subprocess
pub struct NvidiaProvider {
    name: String,
    total_mb: u64,
}

impl NvidiaProvider {
    pub fn detect() -> Option<Self> {
        // Detection runs at startup (blocking is fine here)
        let output = std::process::Command::new("nvidia-smi")
            .args([
                "--query-gpu=name,memory.total",
                "--format=csv,noheader,nounits",
            ])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let line = stdout.lines().next()?;
        let mut parts = line.splitn(2, ',');
        let name = parts.next()?.trim().to_string();
        let total_mb: u64 = parts.next()?.trim().parse().ok()?;

        Some(NvidiaProvider { name, total_mb })
    }

    fn query_used_mb(&self) -> Option<u64> {
        // NOTE: This runs inside spawn_blocking from snapshot() to avoid blocking the async runtime.
        let output = std::process::Command::new("nvidia-smi")
            .args(["--query-gpu=memory.used", "--format=csv,noheader,nounits"])
            .output()
            .ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.lines().next()?.trim().parse().ok()
    }
}

impl MemoryProvider for NvidiaProvider {
    fn id(&self) -> &str {
        "nvidia"
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn kind(&self) -> GpuKind {
        GpuKind::Nvidia
    }

    /// Called from a tokio::task::spawn_blocking context in aggregate_snapshot_async.
    fn snapshot(&self) -> Option<(u64, u64, u64)> {
        let used = self.query_used_mb().unwrap_or(0);
        let free = self.total_mb.saturating_sub(used);
        Some((self.total_mb, used, free))
    }
}
