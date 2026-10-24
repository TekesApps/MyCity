//! Bounded process protocol shared by the CLI supervisor and its integration tests.
pub mod air_turns;
pub mod air_page_metrics;
pub mod air_page_policy;
pub mod adaptive_client;
pub mod compute_worker;

pub mod sim_host;
pub mod worker_cache;

pub mod sim_client;
pub mod sim_probe;

pub mod joint_reference;

pub mod group_sampler;

pub mod joint_probe;

pub mod scale_reference;

pub mod layout_load;

pub mod scale_metrics;

pub mod scale_storage;

pub mod scale_sim;

pub mod cpu_log;

pub mod scale_rare;

pub mod joint_driver;

pub mod heap_meter;

pub mod worker_slots;

pub mod scale_model_task;

pub mod control_data;

pub mod diagnostic_contract;

pub mod normal_pool;

pub mod scale_endurance;

pub mod scale_speed;

pub mod pe0_speed;

mod model_compare;

pub mod steady;

mod observer_clock;

pub mod native_observer;

#[cfg(target_os = "macos")]
pub mod native_watch;

pub mod r07_runtime;

pub mod r07_exercise;

pub(crate) mod vm_trace;

// Real pending-job hints; the original required dispatcher remains authoritative.
pub(crate) mod task_registry_adapter;
