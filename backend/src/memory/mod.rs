use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub mod apple;
pub mod amd;
pub mod intel;
pub mod nvidia;
pub mod system_ram;

/// What kind of memory this provider represents
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GpuKind {
    Nvidia,
    Amd,
    AppleSilicon,
    Intel,
    SystemRam,
}

/// Snapshot of a single memory provider's current state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshot {
    pub provider_id: String,
    pub name: String,
    pub kind: GpuKind,
    pub total_mb: u64,
    pub used_mb: u64,
    pub free_mb: u64,
    pub allocated_mb: u64, // sum of all device allocations from this provider
}

/// Trait every memory provider must implement.
/// `snapshot()` may call blocking subprocesses — it should only be invoked
/// inside `tokio::task::spawn_blocking`.
pub trait MemoryProvider: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn kind(&self) -> GpuKind;
    /// Returns (total_mb, used_mb, free_mb). Returns None if unavailable.
    fn snapshot(&self) -> Option<(u64, u64, u64)>;
}

/// Detect all available providers on this machine (runs at startup, blocking is fine)
pub fn detect_providers() -> Vec<Arc<dyn MemoryProvider>> {
    let mut providers: Vec<Arc<dyn MemoryProvider>> = Vec::new();
    let mut has_apple_silicon = false;

    // NVIDIA
    if let Some(p) = nvidia::NvidiaProvider::detect() {
        tracing::info!("Detected NVIDIA GPU: {}", p.name());
        providers.push(Arc::new(p));
    }

    // AMD
    if let Some(p) = amd::AmdProvider::detect() {
        tracing::info!("Detected AMD GPU: {}", p.name());
        providers.push(Arc::new(p));
    }

    // Apple Silicon (macOS only)
    #[cfg(target_os = "macos")]
    if let Some(p) = apple::AppleProvider::detect() {
        tracing::info!("Detected Apple Silicon: {}", p.name());
        has_apple_silicon = true;
        providers.push(Arc::new(p));
    }

    // Intel integrated
    if let Some(p) = intel::IntelProvider::detect() {
        tracing::info!("Detected Intel iGPU: {}", p.name());
        providers.push(Arc::new(p));
    }

    // System RAM as fallback — skip on Apple Silicon where unified memory IS system RAM
    if has_apple_silicon {
        tracing::info!("Skipping system RAM provider: Apple Silicon unified memory already covers it");
    } else {
        let ram = system_ram::SystemRamProvider::new();
        tracing::info!("System RAM provider: {}", ram.name());
        providers.push(Arc::new(ram));
    }

    providers
}

/// Aggregate snapshot across all providers.
/// Runs provider `snapshot()` calls inside `spawn_blocking` to avoid
/// blocking the async runtime with subprocess calls (nvidia-smi, rocm-smi, vm_stat).
pub async fn aggregate_snapshot_async(providers: &[Arc<dyn MemoryProvider>]) -> Vec<MemorySnapshot> {
    let providers_clone: Vec<Arc<dyn MemoryProvider>> = providers.to_vec();
    tokio::task::spawn_blocking(move || {
        providers_clone
            .iter()
            .filter_map(|p| {
                p.snapshot().map(|(total, used, free)| MemorySnapshot {
                    provider_id: p.id().to_string(),
                    name: p.name().to_string(),
                    kind: p.kind(),
                    total_mb: total,
                    used_mb: used,
                    free_mb: free,
                    allocated_mb: 0, // filled in by API layer from DB
                })
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// Synchronous aggregate snapshot — only safe to call from within spawn_blocking.
/// Kept as a utility for tests or CLI tools; suppress the dead_code warning.
#[allow(dead_code)]
pub fn aggregate_snapshot(providers: &[Arc<dyn MemoryProvider>]) -> Vec<MemorySnapshot> {
    providers
        .iter()
        .filter_map(|p| {
            p.snapshot().map(|(total, used, free)| MemorySnapshot {
                provider_id: p.id().to_string(),
                name: p.name().to_string(),
                kind: p.kind(),
                total_mb: total,
                used_mb: used,
                free_mb: free,
                allocated_mb: 0,
            })
        })
        .collect()
}
