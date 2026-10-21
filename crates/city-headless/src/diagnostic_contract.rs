//! Common diagnostic capacity contract; native/stack/model work is reported separately.
use serde::Serialize;
pub const MIB: usize = 1024 * 1024;
pub const SIM: usize = 5 * MIB / 2;
pub const WORKER: usize = MIB;
pub const LAYOUT: usize = 3 * MIB / 4;
pub const POOL: usize = MIB / 4;
pub const OBSERVER: usize = 2 * MIB;
pub const SUPERVISOR: usize = MIB / 4;
pub const DRIVER: usize = MIB;
pub const HELPER: usize = MIB / 4;
/// Sim and model/stream helper diagnostics now suballocate from their normal
/// role arena. They cannot also subtract external L1 pressure a second time.
pub const EXTERNAL_WITHOUT_OBSERVER: usize = WORKER + LAYOUT + SUPERVISOR + DRIVER + POOL;
#[derive(Serialize)]
pub struct Caps {
    pub sim: usize,
    pub worker: usize,
    pub layout: usize,
    pub observer: usize,
    pub supervisor: usize,
    pub driver: usize,
    pub helper: usize,
    pub pool: usize,
    pub total: usize,
}
pub fn caps() -> Caps {
    Caps {
        sim: SIM,
        worker: WORKER,
        layout: LAYOUT,
        observer: OBSERVER,
        supervisor: SUPERVISOR,
        driver: DRIVER,
        helper: HELPER,
        pool: POOL,
        total: SIM + WORKER + LAYOUT + OBSERVER + SUPERVISOR + DRIVER + HELPER + POOL,
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn total_stays_eight_mib() {
        assert_eq!(super::caps().total, 8 * super::MIB);
        assert_eq!(super::HELPER, crate::control_data::HELPER_CAP);
    }
}
