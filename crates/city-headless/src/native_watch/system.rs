//! Read only named public sysctls. Opaque bytes are decoded without pointer casts.
use super::err;
use serde::Serialize;
use sysctl::{Ctl, CtlValue, Sysctl};
fn value(name: &str) -> Result<CtlValue, String> {
    Ctl::new(name).map_err(err)?.value().map_err(err)
}
pub(super) fn number(name: &str) -> Result<u64, String> {
    match value(name)? {
        CtlValue::Int(v) | CtlValue::S32(v) => u64::try_from(v).map_err(err),
        CtlValue::Uint(v) | CtlValue::U32(v) => Ok(u64::from(v)),
        CtlValue::Long(v) | CtlValue::S64(v) => u64::try_from(v).map_err(err),
        CtlValue::Ulong(v) | CtlValue::U64(v) => Ok(v),
        _ => Err(format!("native_sysctl_type:{name}")),
    }
}
fn text(name: &str) -> Result<String, String> {
    match value(name)? {
        CtlValue::String(v) if v.len() <= 256 => Ok(v),
        _ => Err(format!("native_sysctl_text:{name}")),
    }
}
#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct Critical {
    pub pressure: u64,
    pub total: u64,
    pub available: u64,
    pub used: u64,
    pub page_size: u32,
    pub encrypted: u32,
}
fn swap(bytes: &[u8], pressure: u64) -> Result<Critical, String> {
    if bytes.len() != 32 || ![1, 2, 4].contains(&pressure) {
        return Err("native_swap_or_pressure_abi".into());
    }
    let u64at = |i| u64::from_ne_bytes(bytes[i..i + 8].try_into().expect("checked size"));
    let u32at = |i| u32::from_ne_bytes(bytes[i..i + 4].try_into().expect("checked size"));
    let result = Critical {
        pressure,
        total: u64at(0),
        available: u64at(8),
        used: u64at(16),
        page_size: u32at(24),
        encrypted: u32at(28),
    };
    if result.page_size == 0
        || !result.page_size.is_power_of_two()
        || result.used > result.total
        || result.available > result.total
        || result.encrypted > 1
    {
        return Err("native_swap_values".into());
    }
    Ok(result)
}
pub(super) fn critical() -> Result<Critical, String> {
    let pressure = number("kern.memorystatus_vm_pressure_level")?;
    match value("vm.swapusage")? {
        CtlValue::Struct(b) => swap(&b, pressure),
        _ => Err("native_swap_struct".into()),
    }
}
#[derive(Serialize)]
pub(super) struct Host {
    pub hardware_model: String,
    pub cpu: String,
    pub logical_cpus: u64,
    pub physical_memory_bytes: u64,
    pub os: String,
    pub architecture: &'static str,
    pub page_size_bytes: u64,
}
pub(super) fn host() -> Result<Host, String> {
    Ok(Host {
        hardware_model: text("hw.model")?,
        cpu: text("machdep.cpu.brand_string")?,
        logical_cpus: number("hw.logicalcpu")?,
        physical_memory_bytes: number("hw.memsize")?,
        os: text("kern.osversion")?,
        architecture: std::env::consts::ARCH,
        page_size_bytes: number("hw.pagesize")?,
    })
}
#[derive(Serialize)]
pub(super) struct Vm {
    pub method: &'static str,
    pub free_pages: Option<u64>,
    pub compressor_bytes_used: Option<u64>,
    pub compressor_input_bytes: Option<u64>,
    pub compressor_output_bytes: Option<u64>,
    pub compressor_swapins: Option<u64>,
    pub compressor_swapouts: Option<u64>,
    pub units: &'static str,
}
pub(super) fn vm() -> Vm {
    Vm {
        method: "named sysctl counters, not host_statistics64",
        free_pages: number("vm.page_free_count").ok(),
        compressor_bytes_used: number("vm.compressor_bytes_used").ok(),
        compressor_input_bytes: number("vm.compressor_input_bytes").ok(),
        compressor_output_bytes: number("vm.compressor_compressed_bytes").ok(),
        compressor_swapins: number("vm.compressor.swapper.swapins_total").ok(),
        compressor_swapouts: number("vm.compressor.swapper.swapouts_total").ok(),
        units: "pages/bytes/cumulative compressor events as field names; null means unavailable",
    }
}
pub(super) struct Guard {
    initial_swap: u64,
    pressure_started: Option<u64>,
    last_ns: u64,
    pub maximum_non_normal_ns: u64,
    pub maximum_swap_growth: u64,
}
impl Guard {
    pub fn new(swap: u64) -> Self {
        Self {
            initial_swap: swap,
            pressure_started: None,
            last_ns: 0,
            maximum_non_normal_ns: 0,
            maximum_swap_growth: 0,
        }
    }
    pub fn observe(&mut self, now: u64, value: Critical) -> Result<(), String> {
        if now < self.last_ns {
            return Err("native_guard_clock_reversed".into());
        }
        if value.pressure != 1 && self.pressure_started.is_none() {
            self.pressure_started = Some(self.last_ns);
        }
        if let Some(began) = self.pressure_started {
            self.maximum_non_normal_ns = self.maximum_non_normal_ns.max(now - began);
            if value.pressure == 1 {
                self.pressure_started = None;
            }
        }
        self.last_ns = now;
        self.maximum_swap_growth = self
            .maximum_swap_growth
            .max(value.used.saturating_sub(self.initial_swap));
        if self.maximum_swap_growth > 256 * 1024 * 1024 {
            return Err("native_swap_growth_limit".into());
        }
        if self.maximum_non_normal_ns > 60_000_000_000 {
            return Err("native_pressure_duration_limit".into());
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opaque_layout_length_endian_and_pressure_reject() {
        let mut b = [0; 32];
        b[..8].copy_from_slice(&100u64.to_ne_bytes());
        b[8..16].copy_from_slice(&60u64.to_ne_bytes());
        b[16..24].copy_from_slice(&40u64.to_ne_bytes());
        b[24..28].copy_from_slice(&16384u32.to_ne_bytes());
        assert_eq!(swap(&b, 1).unwrap().used, 40);
        assert!(swap(&b[..31], 1).is_err());
        assert!(swap(&b, 0).is_err());
        b[16..24].copy_from_slice(&101u64.to_ne_bytes());
        assert!(swap(&b, 1).is_err());
    }
    #[test]
    fn consecutive_pressure_and_swap_are_guarded_per_capture() {
        let mut v = Critical {
            pressure: 1,
            total: 1 << 32,
            available: 1 << 32,
            used: 0,
            page_size: 16384,
            encrypted: 1,
        };
        let mut g = Guard::new(0);
        g.observe(1_000_000_000, v).unwrap();
        v.pressure = 2;
        g.observe(2_000_000_000, v).unwrap();
        assert!(g.observe(61_000_000_001, v).is_err());
        let mut g = Guard::new(0);
        v.pressure = 1;
        v.used = 256 * 1024 * 1024 + 1;
        assert!(g.observe(1, v).is_err());
    }
}
