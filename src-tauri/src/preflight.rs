use serde::Serialize;
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
pub struct SystemPreflight {
    pub ram_gb: f64,
    pub vram_gb: Option<f64>,
    pub avx2: bool,
    pub cuda: bool,
    pub metal: bool,
    pub recommended_profile: String,
    pub vision_supported: bool,
    pub warnings: Vec<String>,
}

pub fn run() -> SystemPreflight {
    let ram_gb = total_ram_gb().unwrap_or(0.0);
    let vram_gb = vram_gb();
    let avx2 = avx2_supported();
    let cuda = cuda_supported();
    let metal = cfg!(target_os = "macos");
    let mut warnings = Vec::new();

    if ram_gb > 0.0 && ram_gb < 8.0 {
        warnings.push("الذاكرة أقل من 8GB؛ استخدم نماذج 4-bit فقط وقد يكون Vision بطيئاً.".into());
    }
    // AVX2 is irrelevant on Apple Silicon — Metal handles acceleration instead.
    #[cfg(not(target_arch = "aarch64"))]
    if !avx2 {
        warnings.push("المعالج لا يعلن دعم AVX2؛ سيتم تجنب إعدادات ثقيلة.".into());
    }
    // On macOS, unified memory serves as both RAM and VRAM — skip the VRAM warning.
    #[cfg(not(target_os = "macos"))]
    if vram_gb.unwrap_or(0.0) < 4.0 {
        warnings.push("ذاكرة كرت الشاشة منخفضة أو غير معروفة؛ سيتم التشغيل على CPU عند الحاجة.".into());
    }

    // On macOS, Metal replaces CUDA/VRAM — profile based on unified RAM only.
    let recommended_profile = profile_for(ram_gb, vram_gb, metal).to_string();

    // On Apple Silicon, Metal is the accelerator — AVX2 absence is not a blocker.
    let vision_supported = vision_supported_for(ram_gb, avx2, metal);
    if !vision_supported {
        warnings.push("المعالجة البصرية المحلية قد لا تكون مستقرة على هذا الجهاز.".into());
    }

    SystemPreflight {
        ram_gb,
        vram_gb,
        avx2,
        cuda,
        metal,
        recommended_profile,
        vision_supported,
        warnings,
    }
}

fn total_ram_gb() -> Option<f64> {
    #[cfg(windows)]
    {
        let mut cmd = hidden_command("powershell");
        let out = cmd
            .args([
                "-NoProfile",
                "-Command",
                "(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory",
            ])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let bytes = text.trim().parse::<f64>().ok()?;
        return Some((bytes / 1_073_741_824.0 * 10.0).round() / 10.0);
    }
    #[cfg(target_os = "macos")]
    {
        let out = Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let bytes = text.trim().parse::<f64>().ok()?;
        return Some((bytes / 1_073_741_824.0 * 10.0).round() / 10.0);
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        None
    }
}

fn vram_gb() -> Option<f64> {
    if let Ok(out) = hidden_command("nvidia-smi")
        .args(["--query-gpu=memory.total", "--format=csv,noheader,nounits"])
        .output()
    {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            if let Some(first) = text.lines().next() {
                if let Ok(mib) = first.trim().parse::<f64>() {
                    return Some((mib / 1024.0 * 10.0).round() / 10.0);
                }
            }
        }
    }

    #[cfg(windows)]
    {
        let mut cmd = hidden_command("powershell");
        let out = cmd
            .args([
                "-NoProfile",
                "-Command",
                "(Get-CimInstance Win32_VideoController | Sort-Object AdapterRAM -Descending | Select-Object -First 1 -ExpandProperty AdapterRAM)",
            ])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let bytes = text.trim().parse::<f64>().ok()?;
        if bytes > 0.0 {
            return Some((bytes / 1_073_741_824.0 * 10.0).round() / 10.0);
        }
    }
    None
}

fn avx2_supported() -> bool {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        std::is_x86_feature_detected!("avx2")
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        false
    }
}

fn cuda_supported() -> bool {
    hidden_command("nvidia-smi")
        .arg("-L")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn profile_for(ram_gb: f64, vram_gb: Option<f64>, metal: bool) -> &'static str {
    if metal {
        if ram_gb >= 24.0 { "high-q5-vision" }
        else if ram_gb >= 12.0 { "balanced-q4" }
        else { "safe-q4-low-memory" }
    } else if ram_gb >= 24.0 && vram_gb.unwrap_or(0.0) >= 8.0 {
        "high-q5-vision"
    } else if ram_gb >= 12.0 {
        "balanced-q4"
    } else {
        "safe-q4-low-memory"
    }
}

fn vision_supported_for(ram_gb: f64, avx2: bool, metal: bool) -> bool {
    if metal { ram_gb >= 8.0 } else { ram_gb >= 8.0 && avx2 }
}

fn hidden_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── profile_for ─────────────────────────────────────────────────────────────

    #[test]
    fn profile_metal_high_ram_gives_high_q5() {
        assert_eq!(profile_for(24.0, None, true), "high-q5-vision");
    }

    #[test]
    fn profile_metal_exactly_24gb_boundary() {
        assert_eq!(profile_for(24.0, None, true), "high-q5-vision");
        assert_eq!(profile_for(23.9, None, true), "balanced-q4");
    }

    #[test]
    fn profile_metal_medium_ram_gives_balanced() {
        assert_eq!(profile_for(12.0, None, true), "balanced-q4");
        assert_eq!(profile_for(16.0, None, true), "balanced-q4");
    }

    #[test]
    fn profile_metal_exactly_12gb_boundary() {
        assert_eq!(profile_for(12.0, None, true), "balanced-q4");
        assert_eq!(profile_for(11.9, None, true), "safe-q4-low-memory");
    }

    #[test]
    fn profile_metal_low_ram_gives_safe() {
        assert_eq!(profile_for(4.0, None, true), "safe-q4-low-memory");
        assert_eq!(profile_for(8.0, None, true), "safe-q4-low-memory");
    }

    #[test]
    fn profile_non_metal_high_ram_high_vram_gives_high_q5() {
        assert_eq!(profile_for(24.0, Some(8.0), false), "high-q5-vision");
        assert_eq!(profile_for(32.0, Some(12.0), false), "high-q5-vision");
    }

    #[test]
    fn profile_non_metal_high_ram_but_low_vram_falls_to_balanced() {
        assert_eq!(profile_for(24.0, Some(4.0), false), "balanced-q4");
        assert_eq!(profile_for(24.0, None, false), "balanced-q4");
    }

    #[test]
    fn profile_non_metal_medium_ram_gives_balanced() {
        assert_eq!(profile_for(16.0, Some(4.0), false), "balanced-q4");
        assert_eq!(profile_for(12.0, None, false), "balanced-q4");
    }

    #[test]
    fn profile_non_metal_low_ram_gives_safe() {
        assert_eq!(profile_for(8.0, None, false), "safe-q4-low-memory");
        assert_eq!(profile_for(4.0, None, false), "safe-q4-low-memory");
    }

    #[test]
    fn profile_vram_exactly_8gb_boundary() {
        assert_eq!(profile_for(24.0, Some(8.0), false), "high-q5-vision");
        assert_eq!(profile_for(24.0, Some(7.9), false), "balanced-q4");
    }

    // ── vision_supported_for ────────────────────────────────────────────────────

    #[test]
    fn vision_metal_8gb_supported() {
        assert!(vision_supported_for(8.0, false, true));
        assert!(vision_supported_for(16.0, false, true));
        assert!(vision_supported_for(32.0, false, true));
    }

    #[test]
    fn vision_metal_below_8gb_not_supported() {
        assert!(!vision_supported_for(7.9, false, true));
        assert!(!vision_supported_for(4.0, false, true));
    }

    #[test]
    fn vision_metal_exactly_8gb_boundary() {
        assert!(vision_supported_for(8.0, false, true));
        assert!(!vision_supported_for(7.99, false, true));
    }

    #[test]
    fn vision_non_metal_avx2_8gb_supported() {
        assert!(vision_supported_for(8.0, true, false));
        assert!(vision_supported_for(16.0, true, false));
    }

    #[test]
    fn vision_non_metal_no_avx2_not_supported_even_with_ram() {
        assert!(!vision_supported_for(16.0, false, false));
        assert!(!vision_supported_for(32.0, false, false));
    }

    #[test]
    fn vision_non_metal_avx2_but_low_ram_not_supported() {
        assert!(!vision_supported_for(4.0, true, false));
        assert!(!vision_supported_for(7.9, true, false));
    }
}
