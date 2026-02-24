use super::{GpuKind, MemoryProvider};

/// Apple Silicon unified memory via sysctl.
/// Only activates on Macs with Apple Silicon (ARM) CPUs.
pub struct AppleProvider {
    name: String,
    total_mb: u64,
}

impl AppleProvider {
    #[cfg(target_os = "macos")]
    pub fn detect() -> Option<Self> {
        // Get hardware model string (e.g. "Mac14,3")
        let model_out = std::process::Command::new("sysctl")
            .args(["-n", "hw.model"])
            .output()
            .ok()?;
        let model = String::from_utf8_lossy(&model_out.stdout)
            .trim()
            .to_string();

        // Confirm this is Apple Silicon by checking the CPU brand string.
        // On Apple Silicon this reads "Apple M1" / "Apple M2" / etc.
        // On Intel Macs it reads "Intel(R) Core(TM) i9-..." and the key exists.
        // If the key is absent entirely, fall back to checking hw.cputype == 16777228 (ARM64).
        let is_apple_silicon = {
            let brand = std::process::Command::new("sysctl")
                .args(["-n", "machdep.cpu.brand_string"])
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();

            if !brand.is_empty() {
                // Key exists: Apple Silicon says "Apple Mx", Intel says "Intel(R)..."
                brand.starts_with("Apple")
            } else {
                // Key absent — likely ARM where the key doesn't exist; confirm via cputype
                // hw.cputype 16777228 == CPU_TYPE_ARM64
                std::process::Command::new("sysctl")
                    .args(["-n", "hw.cputype"])
                    .output()
                    .ok()
                    .and_then(|o| {
                        String::from_utf8_lossy(&o.stdout)
                            .trim()
                            .parse::<u32>()
                            .ok()
                    })
                    .map(|t| t == 16777228)
                    .unwrap_or(false)
            }
        };

        if !is_apple_silicon {
            tracing::debug!(
                "AppleProvider: not Apple Silicon (model: {}), skipping",
                model
            );
            return None;
        }

        // Get physical memory via sysctl hw.memsize
        let mem_out = std::process::Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()?;
        let total_bytes: u64 = String::from_utf8_lossy(&mem_out.stdout)
            .trim()
            .parse()
            .ok()?;

        if total_bytes == 0 {
            return None;
        }

        Some(AppleProvider {
            name: format!("Apple Silicon ({model}) Unified Memory"),
            total_mb: total_bytes / (1024 * 1024),
        })
    }

    fn query_used_mb(&self) -> u64 {
        // Use vm_stat to calculate used memory.
        // Page size on Apple Silicon is 16 KiB.
        let out = match std::process::Command::new("vm_stat").output() {
            Ok(o) => o,
            Err(_) => return 0,
        };

        let s = String::from_utf8_lossy(&out.stdout);
        // Read actual page size from the header line: "Mach Virtual Memory Statistics: (page size of 16384 bytes)"
        let page_size: u64 = s
            .lines()
            .next()
            .and_then(|l| {
                let start = l.find("page size of ")? + "page size of ".len();
                let end = l[start..].find(' ')?;
                l[start..start + end].parse().ok()
            })
            .unwrap_or(16384);

        let mut pages_wired: u64 = 0;
        let mut pages_active: u64 = 0;
        let mut pages_occupied: u64 = 0;

        for line in s.lines() {
            let line = line.trim();
            if line.starts_with("Pages wired down:") {
                pages_wired = extract_pages(line);
            } else if line.starts_with("Pages active:") {
                pages_active = extract_pages(line);
            } else if line.starts_with("Pages occupied by compressor:") {
                pages_occupied = extract_pages(line);
            }
        }

        let used_bytes = (pages_wired + pages_active + pages_occupied) * page_size;
        used_bytes / (1024 * 1024)
    }
}

fn extract_pages(line: &str) -> u64 {
    line.split(':')
        .nth(1)
        .map(|s| s.trim().trim_end_matches('.').replace(',', ""))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

impl MemoryProvider for AppleProvider {
    fn id(&self) -> &str {
        "apple"
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn kind(&self) -> GpuKind {
        GpuKind::AppleSilicon
    }

    fn snapshot(&self) -> Option<(u64, u64, u64)> {
        let used = self.query_used_mb();
        let free = self.total_mb.saturating_sub(used);
        Some((self.total_mb, used, free))
    }
}
