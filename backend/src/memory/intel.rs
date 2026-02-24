use super::{GpuKind, MemoryProvider};

/// Intel integrated GPU via sysfs (Linux) or system_profiler (macOS).
/// On Linux, the iGPU shares system RAM and doesn't expose precise usage
/// via standard sysfs — we use GuC/lmem sysfs when available, otherwise
/// report used as a conservative fraction of the shared pool.
pub struct IntelProvider {
    name: String,
    total_mb: u64,
    /// Optional sysfs path for used memory (Linux only)
    #[allow(dead_code)]
    lmem_used_path: Option<std::path::PathBuf>,
}

impl IntelProvider {
    pub fn detect() -> Option<Self> {
        // Linux: look for i915 / xe (Xe2) driver in /sys/class/drm
        #[cfg(target_os = "linux")]
        {
            let drm_path = std::path::Path::new("/sys/class/drm");
            if drm_path.exists() {
                for entry in std::fs::read_dir(drm_path).ok()?.flatten() {
                    let driver_link = entry.path().join("device/driver");
                    if let Ok(link) = std::fs::read_link(&driver_link) {
                        let link_str = link.to_string_lossy();
                        if link_str.contains("i915") || link_str.contains("xe") {
                            // Intel iGPU shares system RAM; report a portion as "VRAM"
                            let mut sys = sysinfo::System::new();
                            sys.refresh_memory();
                            let total_mb = sys.total_memory() / (1024 * 1024);
                            let igpu_mb = total_mb / 2;

                            // Check for lmem (discrete-style local memory) usage path
                            let lmem_path = entry.path().join("device/drm/card0/lmem0/used");
                            let lmem_used_path = if lmem_path.exists() {
                                Some(lmem_path)
                            } else {
                                None
                            };

                            return Some(IntelProvider {
                                name: "Intel Integrated GPU".into(),
                                total_mb: igpu_mb,
                                lmem_used_path,
                            });
                        }
                    }
                }
            }
        }

        // macOS with Intel iGPU (not Apple Silicon)
        #[cfg(target_os = "macos")]
        {
            let out = std::process::Command::new("system_profiler")
                .args(["SPDisplaysDataType", "-json"])
                .output()
                .ok()?;
            let s = String::from_utf8_lossy(&out.stdout);
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&s) {
                if let Some(displays) = json["SPDisplaysDataType"].as_array() {
                    for d in displays {
                        let name = d["spdisplays_device-id"]
                            .as_str()
                            .or_else(|| d["sppci_model"].as_str())
                            .unwrap_or("");
                        if name.to_lowercase().contains("intel") {
                            let vram_str = d["spdisplays_vram"].as_str().unwrap_or("0 MB");
                            let vram_mb: u64 = vram_str
                                .split_whitespace()
                                .next()
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(1536);
                            return Some(IntelProvider {
                                name: format!("Intel iGPU ({})", name),
                                total_mb: vram_mb,
                                lmem_used_path: None,
                            });
                        }
                    }
                }
            }
        }

        None
    }

    fn query_used_mb(&self) -> u64 {
        // Linux: try lmem sysfs first (available on some Gen12+ configs)
        #[cfg(target_os = "linux")]
        if let Some(path) = &self.lmem_used_path {
            if let Ok(s) = std::fs::read_to_string(path) {
                if let Ok(bytes) = s.trim().parse::<u64>() {
                    return bytes / (1024 * 1024);
                }
            }
        }

        // Fallback: read /proc/meminfo to estimate GPU-shared memory usage.
        // MemAvailable gives free+reclaimable. Used ≈ MemTotal - MemAvailable.
        // Since the iGPU shares RAM, this is an approximation.
        #[cfg(target_os = "linux")]
        {
            if let Ok(s) = std::fs::read_to_string("/proc/meminfo") {
                let mut mem_total: u64 = 0;
                let mut mem_available: u64 = 0;
                for line in s.lines() {
                    if line.starts_with("MemTotal:") {
                        mem_total = parse_kb(line);
                    } else if line.starts_with("MemAvailable:") {
                        mem_available = parse_kb(line);
                    }
                }
                if mem_total > 0 {
                    let system_used_mb = mem_total.saturating_sub(mem_available) / 1024;
                    // Attribute a proportional share of system use to the iGPU pool
                    let ratio = self.total_mb as f64 / (mem_total / 1024) as f64;
                    return (system_used_mb as f64 * ratio) as u64;
                }
            }
        }

        // macOS Intel: no simple API for iGPU VRAM usage; return 0
        0
    }
}

#[cfg(target_os = "linux")]
fn parse_kb(line: &str) -> u64 {
    line.split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

impl MemoryProvider for IntelProvider {
    fn id(&self) -> &str {
        "intel"
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn kind(&self) -> GpuKind {
        GpuKind::Intel
    }

    fn snapshot(&self) -> Option<(u64, u64, u64)> {
        let used = self.query_used_mb().min(self.total_mb);
        let free = self.total_mb.saturating_sub(used);
        Some((self.total_mb, used, free))
    }
}
