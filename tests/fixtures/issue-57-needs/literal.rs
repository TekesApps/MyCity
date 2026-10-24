// Handwritten independent arithmetic oracles; no fake host entry or delivery authority.
// token=0, seed=0 => medical day0, police day31, fire day62; u64 maxima checked in u128.
pub const ZERO_PHASES: [u32; 3] = [0, 31, 62];
// 2*(2^64-1) mod90=30, mod180=30; kind offsets add31/62.
pub const MAX_PHASES: [u32; 3] = [30, 61, 92];
pub const EIGHT_AM: u64 = 28_800;
pub const DAY: u64 = 86_400;
