//! Bounded process protocol shared by the CLI supervisor and its integration tests.
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
