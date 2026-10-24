// Handwritten contract oracle, not actual host/resource/qualification/utility evidence.
pub const TOTAL_CENTS: [u64;3] = [3000,2000,5000];
pub const WORK_SECONDS: [u64;3] = [3600,3600,1800];
pub const ENERGY_MWH: [u64;3] = [100000,100000,1000000];
pub const WATER_ML: [u64;3] = [20000,10000,1000000];
// Police 1s + 3599s with exact persisted rational carry.
pub const POLICE_PREFIXES: [(u64,u64,u64,u64);2] = [(1,27,2,0),(3599,99973,9998,1200)];
pub const POLICE_DISPATCH_CENTS:u64=400;
pub const POLICE_RESET_CENTS:u64=400;
