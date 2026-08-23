use serde::{Deserialize, Serialize};

/// Capability tier - simple, deterministic, debuggable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HardwareTier {
    /// CPU-only, smallest models, conservative concurrency
    Tier0,
    /// CPU + acceleration (e.g. AVX2/NEON, optional GPU)
    Tier1,
    /// Capable GPU, optional batching
    Tier2,
}

impl HardwareTier {
    pub fn concurrency(&self) -> usize {
        match self {
            Self::Tier0 => 2,
            Self::Tier1 => 4,
            Self::Tier2 => 8,
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::Tier0 => "Tier0-CPU",
            Self::Tier1 => "Tier1-Accelerated",
            Self::Tier2 => "Tier2-GPU",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub os: String,
    pub arch: String,
    pub tier: HardwareTier,
    pub cpu_count: usize,
    pub ram_mb: Option<u64>,
    pub gpu_present: bool,
}

impl HardwareInfo {
    pub fn detect() -> Self {
        let os = std::env::consts::OS.to_string();
        let arch = std::env::consts::ARCH.to_string();
        let cpu_count = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2);
        // ponytail: no giant hardware DB - simple heuristics
        let gpu_present = detect_gpu_hint();
        let ram_mb = detect_ram_mb();
        let tier = if gpu_present && cpu_count >= 4 {
            HardwareTier::Tier2
        } else if cpu_count >= 4 {
            HardwareTier::Tier1
        } else {
            HardwareTier::Tier0
        };
        Self {
            os,
            arch,
            tier,
            cpu_count,
            ram_mb,
            gpu_present,
        }
    }
}

fn detect_gpu_hint() -> bool {
    if std::env::var("BROOMED_FORCE_GPU").is_ok() {
        return true;
    }
    #[cfg(target_os = "linux")]
    if std::path::Path::new("/dev/dri").exists() {
        return true;
    }
    false
}

fn detect_ram_mb() -> Option<u64> {
    // ponytail: best-effort, never fails detection
    #[cfg(target_os = "linux")]
    {
        if let Ok(s) = std::fs::read_to_string("/proc/meminfo") {
            for line in s.lines() {
                if line.starts_with("MemTotal:") {
                    let kb: u64 = line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0);
                    if kb > 0 {
                        return Some(kb / 1024);
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detect_runs() {
        let info = HardwareInfo::detect();
        assert!(!info.os.is_empty());
        assert!(!info.arch.is_empty());
        assert!(info.cpu_count >= 1);
    }
    #[test]
    fn tier_concurrency() {
        assert_eq!(HardwareTier::Tier0.concurrency(), 2);
        assert_eq!(HardwareTier::Tier2.concurrency(), 8);
    }
}
