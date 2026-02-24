use super::{GpuKind, MemoryProvider};

/// AMD GPU via rocm-smi subprocess
pub struct AmdProvider {
    name: String,
    total_mb: u64,
}

impl AmdProvider {
    pub fn detect() -> Option<Self> {
        // Try rocm-smi first
        let output = std::process::Command::new("rocm-smi")
            .args(["--showmeminfo", "vram", "--json"])
            .output()
            .ok()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&stdout) {
                // rocm-smi JSON: {"card0": {"VRAM Total Memory (B)": "...", ...}}
                if let Some(card) = json.as_object().and_then(|o| o.values().next()) {
                    let total_bytes: u64 = card["VRAM Total Memory (B)"]
                        .as_str()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    if total_bytes > 0 {
                        return Some(AmdProvider {
                            name: "AMD GPU (ROCm)".into(),
                            total_mb: total_bytes / (1024 * 1024),
                        });
                    }
                }
            }
        }

        // Fallback: check /sys/class/drm for amdgpu
        let sys_path = std::path::Path::new("/sys/class/drm");
        if sys_path.exists() {
            for entry in std::fs::read_dir(sys_path).ok()?.flatten() {
                let mem_path = entry.path().join("device/mem_info_vram_total");
                if mem_path.exists() {
                    let bytes: u64 = std::fs::read_to_string(&mem_path)
                        .ok()
                        .and_then(|s| s.trim().parse().ok())
                        .unwrap_or(0);
                    if bytes > 0 {
                        return Some(AmdProvider {
                            name: "AMD GPU (sysfs)".into(),
                            total_mb: bytes / (1024 * 1024),
                        });
                    }
                }
            }
        }

        None
    }

    fn query_used_mb(&self) -> u64 {
        // Try rocm-smi
        if let Ok(out) = std::process::Command::new("rocm-smi")
            .args(["--showmeminfo", "vram", "--json"])
            .output()
        {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout);
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&s) {
                    if let Some(card) = json.as_object().and_then(|o| o.values().next()) {
                        let used_bytes: u64 = card["VRAM Total Used Memory (B)"]
                            .as_str()
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(0);
                        return used_bytes / (1024 * 1024);
                    }
                }
            }
        }

        // sysfs fallback
        let sys_path = std::path::Path::new("/sys/class/drm");
        if let Ok(entries) = std::fs::read_dir(sys_path) {
            for entry in entries.flatten() {
                let mem_path = entry.path().join("device/mem_info_vram_used");
                if let Ok(s) = std::fs::read_to_string(mem_path) {
                    if let Ok(b) = s.trim().parse::<u64>() {
                        return b / (1024 * 1024);
                    }
                }
            }
        }
        0
    }
}

impl MemoryProvider for AmdProvider {
    fn id(&self) -> &str {
        "amd"
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn kind(&self) -> GpuKind {
        GpuKind::Amd
    }

    fn snapshot(&self) -> Option<(u64, u64, u64)> {
        let used = self.query_used_mb();
        let free = self.total_mb.saturating_sub(used);
        Some((self.total_mb, used, free))
    }
}
