mod air_probe;
mod background_probe;
mod boot_probe;
mod continuity;
mod economy_probe;
mod exchange_probe;
mod session;
mod timeline_probe;
mod worker_probe;
use mycity_core::{parse_bootstrap, read_bounded, sha256, INPUT_LIMIT, NUMERIC_STRATEGY, SCHEMA};
use serde_json::json;
use std::{
    env,
    fs::File,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn system(command: &str, args: &[&str]) -> Option<String> {
    let result = Command::new(command).args(args).output().ok()?;
    result
        .status
        .success()
        .then(|| String::from_utf8_lossy(&result.stdout).trim().to_owned())
}

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args().collect();
    if args.get(1).is_some_and(|a| a == "economy-short-probe") {
        if args.len() != 5 {
            return Err("usage: mycity-headless economy-short-probe INPUT NEW_DIR debt|prepay|refund|suspend".into());
        }
        let bytes = read_bounded(
            File::open(&args[2]).map_err(|e| e.to_string())?,
            INPUT_LIMIT,
        )
        .map_err(|e| e.to_string())?;
        let report = economy_probe::run_short(&bytes, std::path::Path::new(&args[3]), &args[4])?;
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "economy-short-restore") {
        if args.len() != 3 {
            return Err("usage: mycity-headless economy-short-restore WORLD".into());
        }
        let report = economy_probe::restore_short(std::path::Path::new(&args[2]))?;
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "scale-rare-anchors") {
        if args.len() != 3 {
            return Err("usage: mycity-headless scale-rare-anchors NEW_DIR".into());
        }
        let r = mycity_headless::scale_rare::anchors(std::path::Path::new(&args[2]))?;
        println!(
            "{}",
            serde_json::to_string_pretty(&r).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "scale-rare-trial") {
        if args.len() != 5 {
            return Err(
                "usage: mycity-headless scale-rare-trial ANCHORS NEW_DIR claims|source".into(),
            );
        }
        let r = mycity_headless::scale_rare::trial(
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
            &args[4],
        )?;
        println!(
            "{}",
            serde_json::to_string_pretty(&r).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "scale-rare-restore") {
        if args.len() != 4 {
            return Err("usage: mycity-headless scale-rare-restore WORLD EXPECTED_ENGINE".into());
        }
        let r = mycity_headless::scale_rare::restore(
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
        )?;
        println!(
            "{}",
            serde_json::to_string_pretty(&r).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "layout-load-only") {
        if args.len() != 4 {
            return Err(
                "usage: mycity-headless layout-load-only EXISTING_SNAPSHOT NEW_REPORT_DIR".into(),
            );
        }
        let report = mycity_headless::layout_load::load_only(
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
        )?;
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "layout-reopen") {
        if args.len() != 4 {
            return Err(
                "usage: mycity-headless layout-reopen EXISTING_SNAPSHOT NEW_REPORT_DIR".into(),
            );
        }
        let report = mycity_headless::layout_load::reopen(
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
        )?;
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "layout-charged-probe") {
        if args.len() != 6 {
            return Err("usage: mycity-headless layout-charged-probe NEW_DIR WALL_SECONDS SNAPSHOT_SECONDS EXTERNAL_BYTES".into());
        }
        let report = mycity_headless::layout_load::probe_charged(
            std::path::Path::new(&args[2]),
            args[3]
                .parse()
                .map_err(|e: std::num::ParseIntError| e.to_string())?,
            args[4]
                .parse()
                .map_err(|e: std::num::ParseIntError| e.to_string())?,
            Some(
                args[5]
                    .parse()
                    .map_err(|e: std::num::ParseIntError| e.to_string())?,
            ),
        )?;
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "layout-probe") {
        if args.len() != 5 {
            return Err("usage: mycity-headless layout-probe NEW_DIR WALL_SECONDS SNAPSHOT_SECONDS (>=2100s requires 300s snapshots)".into());
        }
        let report = mycity_headless::layout_load::probe(
            std::path::Path::new(&args[2]),
            args[3]
                .parse()
                .map_err(|e: std::num::ParseIntError| e.to_string())?,
            args[4]
                .parse()
                .map_err(|e: std::num::ParseIntError| e.to_string())?,
        )?;
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "scale-sim-probe") {
        if !(3..=5).contains(&args.len()) {
            return Err(
                "usage: mycity-headless scale-sim-probe NEW_DIR [small|medium|full] [measure]"
                    .into(),
            );
        }
        if args.get(4).is_some_and(|s| s != "measure") {
            return Err("scale_measure_option".into());
        }
        let tier = mycity_headless::scale_reference::Tier::parse(
            args.get(3).map_or("full", String::as_str),
        )?;
        println!(
            "{}",
            serde_json::to_string_pretty(&mycity_headless::scale_sim::run_measured(
                std::path::Path::new(&args[2]),
                tier,
                args.get(4).is_some_and(|s| s == "measure")
            )?)
            .map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|a| {
        matches!(
            a.as_str(),
            "sim-host-scale-full"
                | "sim-host-scale-full-probe"
                | "sim-host-scale-small"
                | "sim-host-scale-small-probe"
                | "sim-host-scale-medium"
                | "sim-host-scale-medium-probe"
        )
    }) {
        if args.len() != 4 || !matches!(args[2].as_str(), "create" | "open") {
            return Err(
                "usage: mycity-headless sim-host-scale-TIER[-probe] create|open WORLD".into(),
            );
        }
        let name = args[1]
            .trim_start_matches("sim-host-scale-")
            .trim_end_matches("-probe");
        let tier = mycity_headless::scale_reference::Tier::parse(name)?;
        return mycity_headless::sim_host::run_scale(
            std::path::Path::new(&args[3]),
            args[2] == "create",
            args[1].ends_with("probe"),
            tier,
        );
    }
    if args.get(1).is_some_and(|a| a == "scale-store-probe-v2") {
        if args.len() != 3 {
            return Err("usage: mycity-headless scale-store-probe-v2 NEW_DIR".into());
        }
        let report = mycity_headless::scale_storage::run_v2(std::path::Path::new(&args[2]))?;
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "scale-store-probe") {
        if args.len() != 3 {
            return Err("usage: mycity-headless scale-store-probe NEW_DIR".into());
        }
        let report = mycity_headless::scale_storage::run(std::path::Path::new(&args[2]))?;
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "scale-reference") {
        if args.len() != 3 {
            return Err("usage: mycity-headless scale-reference NEW_DIR".into());
        }
        let report = mycity_headless::scale_reference::export(std::path::Path::new(&args[2]))?;
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "joint-probe") {
        if args.len() != 4 {
            return Err("usage: mycity-headless joint-probe NEW_DIR WALL_SECONDS (0=dry; 1800..2100=endurance)".into());
        }
        let wall = args[3].parse::<u64>().map_err(|e| e.to_string())?;
        println!(
            "{}",
            serde_json::to_string_pretty(&mycity_headless::joint_probe::run(
                std::path::Path::new(&args[2]),
                wall
            )?)
            .map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args
        .get(1)
        .is_some_and(|a| matches!(a.as_str(), "sim-host-joint" | "sim-host-joint-probe"))
    {
        if args.len() != 4 || !matches!(args[2].as_str(), "create" | "open") {
            return Err("usage: mycity-headless sim-host-joint[-probe] create|open WORLD".into());
        }
        return mycity_headless::sim_host::run_joint(
            std::path::Path::new(&args[3]),
            args[2] == "create",
            args[1].ends_with("probe"),
        );
    }
    if args.get(1).is_some_and(|a| a == "sim-probe") {
        if args.len() != 3 {
            return Err("usage: mycity-headless sim-probe NEW_DIR".into());
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&mycity_headless::sim_probe::run(std::path::Path::new(
                &args[2]
            ))?)
            .map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "sim-host-probe") {
        if args.len() != 4 || !matches!(args[2].as_str(), "create" | "open") {
            return Err("usage: mycity-headless sim-host-probe create|open WORLD".into());
        }
        return mycity_headless::sim_host::run_probe(
            std::path::Path::new(&args[3]),
            args[2] == "create",
        );
    }
    if args
        .get(1)
        .is_some_and(|a| a == "compute-worker-bounded-probe")
    {
        if args.len() != 3 {
            return Err("usage: mycity-headless compute-worker-bounded-probe TASK_ROOT".into());
        }
        return mycity_headless::compute_worker::run_bounded_probe(std::path::Path::new(&args[2]));
    }
    if args.get(1).is_some_and(|a| a == "sim-host") {
        if args.len() != 4 || !matches!(args[2].as_str(), "create" | "open") {
            return Err("usage: mycity-headless sim-host create|open WORLD".into());
        }
        return mycity_headless::sim_host::run(std::path::Path::new(&args[3]), args[2] == "create");
    }
    if args.get(1).is_some_and(|a| a == "background-probe") {
        if args.len() != 3 {
            return Err("usage: mycity-headless background-probe NEW_DIR".into());
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&background_probe::run(std::path::Path::new(&args[2]))?)
                .map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "worker-probe") {
        if args.len() != 3 {
            return Err("usage: mycity-headless worker-probe NEW_DIR".into());
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&worker_probe::run(std::path::Path::new(&args[2]))?)
                .map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args
        .get(1)
        .is_some_and(|a| a == "compute-worker-probe-gated")
    {
        if args.len() != 3 {
            return Err("usage: mycity-headless compute-worker-probe-gated TASK_ROOT".into());
        }
        return mycity_headless::compute_worker::run_probe_gated(std::path::Path::new(&args[2]));
    }
    if args.get(1).is_some_and(|a| a == "compute-worker-bounded") {
        if args.len() != 3 {
            return Err("usage: mycity-headless compute-worker-bounded TASK_ROOT".into());
        }
        return mycity_headless::compute_worker::run_bounded(std::path::Path::new(&args[2]));
    }
    if args.get(1).is_some_and(|a| a == "compute-worker") {
        if args.len() != 3 {
            return Err("usage: mycity-headless compute-worker TASK_ROOT".into());
        }
        return mycity_headless::compute_worker::run(std::path::Path::new(&args[2]));
    }
    if args.get(1).is_some_and(|a| a == "air-probe") {
        if args.len() != 4 {
            return Err("usage: mycity-headless air-probe INPUT NEW_DIR".into());
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&air_probe::run(
                std::path::Path::new(&args[2]),
                std::path::Path::new(&args[3])
            )?)
            .map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "air-resume") {
        if args.len() != 5 {
            return Err("usage: mycity-headless air-resume INPUT CHECKPOINT NEW_OUTPUT".into());
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&air_probe::resume(
                std::path::Path::new(&args[2]),
                std::path::Path::new(&args[3]),
                std::path::Path::new(&args[4])
            )?)
            .map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "timeline-probe") {
        if args.len() != 3 {
            return Err("usage: mycity-headless timeline-probe NEW_DIR".into());
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&timeline_probe::run(std::path::Path::new(&args[2]))?)
                .map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "session") {
        return session::run(&args);
    }
    if args.get(1).is_some_and(|a| a == "continuity-worker") {
        return continuity::worker(&args);
    }
    if args.get(1).is_some_and(|a| a == "continuity-probe") {
        if args.len() != 4 {
            return Err("usage: mycity-headless continuity-probe NEW_DIR WALL_SECONDS (1800..3600 for duration gate)".into());
        }
        let seconds = args[3].parse::<u64>().map_err(|e| e.to_string())?;
        println!(
            "{}",
            serde_json::to_string_pretty(&continuity::run(
                std::path::Path::new(&args[2]),
                seconds
            )?)
            .map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "query") {
        if !(5..=7).contains(&args.len()) {
            return Err("usage: mycity-headless query WORLD RULES <contracts|fulfilment|arrears|capacity|blocked|recovery|receipts|timeline> [1..64] [cursor]".into());
        }
        let query: mycity_storage::Query =
            serde_json::from_value(json!(args[4])).map_err(|e| e.to_string())?;
        let limit = args
            .get(5)
            .map(|s| s.parse::<usize>())
            .transpose()
            .map_err(|e| e.to_string())?
            .unwrap_or(16);
        let mut store = mycity_storage::Store::open(std::path::Path::new(&args[2]), &args[3])
            .map_err(|e| format!("{e:?}"))?;
        let page = store
            .query(query, args.get(6).map(String::as_str), limit)
            .map_err(|e| format!("{e:?}"))?;
        println!(
            "{}",
            serde_json::to_string_pretty(&page).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    let storage = args.get(1).is_some_and(|a| {
        [
            "storage-probe",
            "regional-probe",
            "economy-probe",
            "boot-probe",
            "exchange-probe",
        ]
        .contains(&a.as_str())
    });
    let spatial = args.get(1).is_some_and(|a| a == "spatial-probe");
    let spatial_store = args.get(1).is_some_and(|a| a == "spatial-store-probe");
    if args.len()
        != if spatial_store {
            6
        } else if storage || spatial {
            4
        } else {
            3
        }
        || ![
            "baseline",
            "layout",
            "assemble",
            "transaction-probe",
            "clock-probe",
            "storage-probe",
            "regional-probe",
            "spatial-probe",
            "economy-probe",
            "boot-probe",
            "exchange-probe",
            "spatial-store-probe",
        ]
        .contains(&args[1].as_str())
    {
        return Err(
            "usage: mycity-headless <baseline|layout|assemble|transaction-probe|clock-probe> <input.json>; <storage-probe|regional-probe|economy-probe|boot-probe|exchange-probe> <input.json> <new_world_dir>; spatial-probe <input.json> <height.png>; spatial-store-probe <opening.json> <spatial.json> <height.png> <new_world_dir>"
                .into(),
        );
    }
    let file = File::open(&args[2]).map_err(|e| e.to_string())?;
    let bytes = read_bounded(file, INPUT_LIMIT).map_err(|e| e.to_string())?;
    let assembled = if [
        "assemble",
        "transaction-probe",
        "storage-probe",
        "regional-probe",
        "spatial-store-probe",
        "economy-probe",
        "boot-probe",
        "exchange-probe",
    ]
    .contains(&args[1].as_str())
    {
        Some(mycity_core::assembly::assemble(&bytes)?)
    } else {
        None
    };
    let spatial_input = if spatial {
        Some(
            serde_json::from_slice::<mycity_map::probe::Input>(&bytes)
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    let input = if let Some(s) = &spatial_input {
        s.bootstrap.validate().map_err(str::to_owned)?;
        s.bootstrap.clone()
    } else if let Some(world) = &assembled {
        let view = world.view();
        mycity_core::Bootstrap {
            schema: SCHEMA.into(),
            world: view.scope.world(),
            branch: view.scope.branch(),
            seed: view.seed,
            laboratory_only: true,
        }
    } else {
        parse_bootstrap(&bytes)?
    };
    let mut rules = json!({"schema": SCHEMA, "numeric_strategy": NUMERIC_STRATEGY, "input_limit_bytes": INPUT_LIMIT, "mode": "headless_laboratory"});
    if assembled.is_some() {
        rules["opening_schema"] = json!(mycity_core::assembly::ASSEMBLY_SCHEMA);
        rules["max_accounts"] = json!(mycity_core::assembly::MAX_ACCOUNTS);
        rules["max_stocks"] = json!(mycity_core::assembly::MAX_STOCKS);
        rules["max_sources"] = json!(mycity_core::assembly::MAX_SOURCES);
        rules["world_byte_budget"] = json!(mycity_core::assembly::WORLD_BYTE_BUDGET);
    }
    if args[1] == "exchange-probe" {
        rules["exchange"] = json!({"schema":"mycity-m0-exchange-v1","profile":"regional-fulfilment-v1","clock":"region_interaction_time","max_quotes":8,"max_shipments":4,"loss_per_edge_per_mille":10,"freight_unit_slots_per_Q":20,"freight_cents_per_edge":10,"terrain_template":"shared_flat_z0_all_valid_dry"});
    }
    if args[1] == "boot-probe" {
        rules["boot"] = json!({"schema":"mycity-m0-boot-v1","profile":mycity_core::boot::PROFILE,"duration_seconds":60,"electric_slot_envelope_watts":24000,"water_slot_envelope_liters_per_second":10,"cost_policy":"fuel_frozen_lot_utility_blended_pool","failure":"committed_inputs_not_refunded_worst_outlets_reserved"});
    }
    if args[1] == "economy-probe" {
        rules["economy"] = json!({"profile":"pe0-30d-v1","schema":"mycity-m0-economy-v1","days":mycity_core::economy::DAYS,"max_step_seconds":3600,
            "cost_policy":"direct_labor_capitalized_fifo_frozen_lot_v1","tax_policy":"cumulative_net_profit_floor_10_percent_v1","transport":"one_hour_10Q_in_transit_20Q_daily_synthetic_source"});
    }
    if args[1] == "transaction-probe" {
        rules["transaction"] = json!({"max_actions":mycity_core::transaction::MAX_ACTIONS,"max_receipts":mycity_core::transaction::MAX_RECEIPTS,"engine_byte_budget":mycity_core::transaction::ENGINE_BYTE_BUDGET,"cost_policy":"frozen_lot_interval_v1","protected_release":"trusted_host_only"});
    }
    if args[1] == "regional-probe" {
        rules["regional"] = json!({"schema":"mycity-m0-region-v1","max_cities":mycity_core::region::MAX_CITIES,
            "max_routes":mycity_core::region::MAX_ROUTES,"max_transits":mycity_core::region::MAX_TRANSITS,
            "max_obligations":mycity_core::region::MAX_OBLIGATIONS,"max_shortfall":mycity_core::region::MAX_SHORTFALL,
            "max_advance_seconds":mycity_core::region::MAX_ADVANCE_SECONDS,"contract_clock":"interaction_time",
            "route_model":"whole_journey_people_capacity_reservation_not_vehicle_flow","population":"anonymous_no_personal_financial_rights"});
    }
    if args[1] == "clock-probe" {
        rules["schedule"] = json!({"source_limit":mycity_core::schedule::SOURCE_LIMIT,"ready_limit":mycity_core::schedule::READY_LIMIT,"max_interval_seconds":mycity_core::schedule::MAX_INTERVAL_SECONDS,"calendar_epoch":"2001-01-01","rng":"sha256-domain-source-window-v1"});
    }
    if storage || spatial_store {
        rules["storage"] = json!({"schema":if spatial_store || args[1] == "exchange-probe" {"mycity-m0-genesis-v2"}else if ["economy-probe", "boot-probe"].contains(&args[1].as_str()) {"explicit_v2_spatial_baseline_and_v1_financial_branches"}else{"mycity-m0-genesis-v1"},"rules_identity":env!("MYCITY_SOURCE_SHA256"),"frame_limit":mycity_storage::FRAME_LIMIT,"wal_limit":mycity_storage::WAL_LIMIT,"committed_boundary":"CURRENT_root_hash_sequence_and_wal_end"});
    }
    if spatial || spatial_store {
        rules["spatial"] = json!({"schema":mycity_map::SCHEMA,"land_cell_cm":mycity_map::LAND_CELL_CM,
            "terrain_step_cm":mycity_map::TERRAIN_STEP_CM,"xy_limit_cm":mycity_map::XY_LIMIT_CM,
            "z_limit_cm":mycity_map::Z_LIMIT_CM,"geometry_budget_bytes":mycity_map::GEOMETRY_BUDGET_BYTES,
            "terrain_resident_budget_bytes":mycity_map::TERRAIN_RESIDENT_BUDGET_BYTES,
            "import_peak_budget_bytes":mycity_map::IMPORT_PEAK_BUDGET_BYTES,"source_byte_limit":mycity_map::terrain::SOURCE_BYTE_LIMIT,
            "decoder_byte_limit":mycity_map::terrain::DECODER_BYTE_LIMIT,"max_source_side":mycity_map::terrain::MAX_SOURCE_SIDE,
            "max_vertices":mycity_map::geometry::MAX_VERTICES,"curve_error_cm":mycity_map::geometry::CURVE_ERROR_CM,
            "numeric_strategy":"bounded-i128-bilinear-cm-swne-integer-capsule-v1"});
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let mut report = json!({
        "report_schema": "mycity-m0-evidence-v1",
        "task": "M0-T01", "run_id": format!("{}-{now}", std::process::id()),
        "result": "bootstrap_validated_only",
        "source_sha256": env!("MYCITY_SOURCE_SHA256"), "lock_sha256": env!("MYCITY_LOCK_SHA256"),
        "input_sha256": sha256(&bytes), "rules_sha256": sha256(&serde_json::to_vec(&rules).map_err(|e| e.to_string())?),
        "rules": rules, "input": input,
        "build": {"rustc": env!("MYCITY_RUSTC"), "target": env!("MYCITY_TARGET"), "profile": env!("MYCITY_PROFILE")},
        "host": {"os": env::consts::OS, "arch": env::consts::ARCH,
            "os_version": system("/usr/bin/sw_vers", &["-productVersion"]),
            "memory_bytes": system("/usr/sbin/sysctl", &["-n", "hw.memsize"]),
            "chip": system("/usr/sbin/sysctl", &["-n", "machdep.cpu.brand_string"])},
        "ui": "not_implemented_by_scope", "gpu": "not_implemented",
        "economic_prototypes": "not_run", "memory_16gb_gate": "not_run"
    });
    if args[1] == "layout" {
        use mycity_core::arena::{AccountDomain, Arena, Scope};
        let mut table = Arena::<AccountDomain, u64>::new(
            Scope::new(input.world, input.branch).map_err(|e| format!("{e:?}"))?,
            32768,
            2 * 1024 * 1024,
        )
        .map_err(|e| format!("{e:?}"))?;
        let mut checksum = 0u64;
        for n in 0..32768u64 {
            let h = table.insert(n).map_err(|e| format!("{e:?}"))?;
            checksum += *table.get(h).map_err(|e| format!("{e:?}"))?;
        }
        report["task"] = json!("M0-T02");
        report["result"] = json!("fixed_record_layout_executed");
        report["layout"] = json!({"type": "u64_probe_not_full_financial_record", "table": table.layout(), "readback_sum": checksum, "allocator_metadata_in_table_bytes": false});
    }
    if let Some(world) = assembled {
        report["task"] = json!("M0-T03");
        report["result"] = json!("opening_assembled_not_simulated");
        report["world"] = json!(world.view());
        report["state_sha256"] = json!(world.state_hash());
        report["layout"] = world.layout();
    }
    if args[1] == "transaction-probe" {
        report["task"] = json!("M0-T04");
        report["result"] = json!("volatile_transaction_probes_passed");
        report["transaction_probe"] = mycity_core::probes::transaction_probe(&bytes)?;
    }
    if args[1] == "clock-probe" {
        report["task"] = json!("M0-T05");
        report["result"] = json!("static_clock_probes_passed");
        report["clock_probe"] = mycity_core::probes::schedule_probe(&input)?;
    }
    if args[1] == "storage-probe" {
        report["task"] = json!("M0-T06");
        report["result"] = json!("transaction_storage_roundtrip_passed");
        report["storage_probe"] = storage_probe(&bytes, std::path::Path::new(&args[3]))?;
    }
    if args[1] == "exchange-probe" {
        report["task"] = json!("M0-T09-finite-fulfilment");
        report["exchange_probe"] = exchange_probe::run(
            &bytes,
            std::path::Path::new(&args[3]),
            env!("MYCITY_SOURCE_SHA256"),
        )?;
        report["result"] = json!("finite_regional_fulfilment_executed");
    }
    if args[1] == "boot-probe" {
        report["task"] = json!("M0-T08-P-BOOT");
        report["boot_probe"] = boot_probe::run(
            &bytes,
            std::path::Path::new(&args[3]),
            env!("MYCITY_SOURCE_SHA256"),
        )?;
        report["result"] = json!("finite_startup_interlock_executed");
        report["P_BOOT"] = json!("finite_experiment_executed");
    }
    if args[1] == "economy-probe" {
        report["task"] = json!("M0-T08-P-E0-P-E1");
        report["result"] = json!("economic_prototypes_executed");
        report["economy_probe"] = economy_probe::run(
            &bytes,
            std::path::Path::new(&args[3]),
            env!("MYCITY_SOURCE_SHA256"),
        )?;
        report["economic_prototypes"] = json!("P-E0_and_P-E1_executed");
    }
    if args[1] == "regional-probe" {
        report["task"] = json!("M0-T09-snapshot-subset");
        report["result"] = json!("regional_snapshot_subset_executed");
        report["regional_probe"] = regional_probe(&bytes, std::path::Path::new(&args[3]))?;
    }
    if spatial_store {
        let spatial_bytes = read_bounded(
            File::open(&args[3]).map_err(|e| e.to_string())?,
            INPUT_LIMIT,
        )
        .map_err(|e| e.to_string())?;
        let png = read_bounded(
            File::open(&args[4]).map_err(|e| e.to_string())?,
            mycity_map::terrain::SOURCE_BYTE_LIMIT,
        )
        .map_err(|e| e.to_string())?;
        report["task"] = json!("M0-T07-spatial-storage-subset");
        report["result"] = json!("immutable_spatial_storage_executed");
        report["spatial_input_sha256"] = json!(sha256(&spatial_bytes));
        report["png_input_sha256"] = json!(sha256(&png));
        report["spatial_store_probe"] =
            spatial_store_probe(&bytes, &spatial_bytes, &png, std::path::Path::new(&args[5]))?;
    }
    if let Some(input) = spatial_input {
        let file = File::open(&args[3]).map_err(|e| e.to_string())?;
        let png = read_bounded(file, mycity_map::terrain::SOURCE_BYTE_LIMIT)
            .map_err(|e| e.to_string())?;
        report["task"] = json!("M0-T07-spatial-subset");
        report["result"] = json!("spatial_fixture_executed");
        report["png_input_sha256"] = json!(sha256(&png));
        report["spatial_probe"] =
            mycity_map::probe::run(input, &png).map_err(|e| format!("{e:?}"))?;
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn storage_probe(input: &[u8], dir: &std::path::Path) -> Result<serde_json::Value, String> {
    use mycity_core::{
        transaction::{Action, Caller, Envelope, Resource},
        units::Money,
    };
    use mycity_storage::Store;
    let err = |e| format!("{e:?}");
    let mut store = Store::create(
        dir,
        input,
        vec![Resource::new(1, 10).map_err(|e| format!("{e:?}"))?],
        env!("MYCITY_SOURCE_SHA256"),
    )
    .map_err(err)?;
    let world = store.engine().map_err(err)?.world();
    let view = world.view();
    let family = world.account_ref(2)?;
    let company = world.account_ref(3)?;
    let supplier = world.account_ref(4)?;
    let command = Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id: 1,
        expected_revision: 0,
        actions: vec![
            Action::Protect {
                account: family,
                amount: Money::new(10000).map_err(|e| format!("{e:?}"))?,
            },
            Action::Transfer {
                from: company,
                to: supplier,
                amount: Money::new(10000).map_err(|e| format!("{e:?}"))?,
            },
            Action::ReserveCapacity {
                resource: 1,
                units: 8,
            },
        ],
    };
    let first = store
        .execute(Caller::System, command.clone())
        .map_err(err)?;
    let receipt = first.model.map_err(|e| format!("{e:?}"))?;
    let token = receipt.created_tokens[2].ok_or("missing_capacity_token")?;
    let second = Envelope {
        world: view.scope.world(),
        branch: view.scope.branch(),
        operation_id: 2,
        expected_revision: 1,
        actions: vec![
            Action::OccupyCapacity { token },
            Action::PayProtected {
                from: family,
                to: company,
                amount: Money::new(10000).map_err(|e| format!("{e:?}"))?,
            },
        ],
    };
    let second = store.execute(Caller::System, second).map_err(err)?;
    second.model.map_err(|e| format!("{e:?}"))?;
    let before = store.engine().map_err(err)?.state_hash();
    let journal =
        serde_json::to_vec(store.engine().map_err(err)?.journal()).map_err(|e| e.to_string())?;
    drop(store);
    let mut restored = Store::open(dir, env!("MYCITY_SOURCE_SHA256")).map_err(err)?;
    let replay = restored.execute(Caller::System, command).map_err(err)?;
    let engine = restored.engine().map_err(err)?;
    if !first.has_persistent_receipt
        || !second.has_persistent_receipt
        || !replay.has_persistent_receipt
        || before != engine.state_hash()
        || restored.committed_sequence() != 2
        || serde_json::to_vec(engine.journal()).map_err(|e| e.to_string())? != journal
    {
        return Err("storage_roundtrip_probe_failed".into());
    }
    Ok(
        json!({"result":"committed_transactions_recovered_once","committed_sequence":2,"world":engine.world().view(),"resources":engine.resources(),"state_sha256":before,"journal_sha256":sha256(&journal),"replay_has_persistent_receipt":replay.has_persistent_receipt,"engine_owned_capacity_bytes":engine.owned_capacity_bytes(),"model_receipt_durability":"volatile_only_inside_core; storage_wrapper_confirms_persistent_coverage","scope":"transaction_recovery_only_not_scheduler_checkpoint","power_loss":"not_tested","memory_16gb_gate":"not_run"}),
    )
}

fn regional_probe(input: &[u8], dir: &std::path::Path) -> Result<serde_json::Value, String> {
    use mycity_core::transaction::{Action, Caller, Envelope};
    use mycity_storage::Store;
    fn err(e: impl std::fmt::Debug) -> String {
        format!("{e:?}")
    }
    let mut store = Store::create(dir, input, vec![], env!("MYCITY_SOURCE_SHA256")).map_err(err)?;
    let initial = store.engine().map_err(err)?.world().view();
    let candidates = initial
        .region
        .ok_or("region_required")?
        .candidates(1)
        .map_err(err)?;
    let scope = initial.scope;
    let commands = [
        vec![Action::RegionMigrate {
            from: 1,
            to: 2,
            requested: 2000,
            tolerance: 2,
        }],
        vec![Action::RegionAdvance { seconds: 30 }],
        vec![Action::RegionActivate { city: 1 }],
    ];
    let mut first = None;
    let mut outcomes = Vec::new();
    for (i, actions) in commands.into_iter().enumerate() {
        let command = Envelope {
            world: scope.world(),
            branch: scope.branch(),
            operation_id: i as u64 + 1,
            expected_revision: i as u64,
            actions,
        };
        if i == 0 {
            first = Some(command.clone());
        }
        let result = store.execute(Caller::System, command).map_err(err)?;
        result.model.map_err(err)?;
        if !result.has_persistent_receipt {
            return Err("missing_persistent_receipt".into());
        }
        outcomes.push(result);
    }
    let before = store.engine().map_err(err)?.state_hash();
    drop(store);
    let mut restored = Store::open(dir, env!("MYCITY_SOURCE_SHA256")).map_err(err)?;
    let retry = restored
        .execute(Caller::System, first.ok_or("missing_first")?)
        .map_err(err)?;
    let engine = restored.engine().map_err(err)?;
    let final_view = engine.world().view();
    let r = final_view.region.ok_or("missing_region")?;
    let people = |v: &mycity_core::assembly::WorldView| {
        let r = v.region.expect("region");
        r.cities
            .iter()
            .flatten()
            .map(|c| u64::from(c.population))
            .sum::<u64>()
            + r.transits
                .iter()
                .flatten()
                .map(|t| u64::from(t.actual))
                .sum::<u64>()
    };
    let cash = |v: &mycity_core::assembly::WorldView| {
        v.accounts.iter().map(|a| a.cash.value()).sum::<i64>()
    };
    if before != engine.state_hash()
        || restored.committed_sequence() != 3
        || !retry.has_persistent_receipt
        || retry.model.map_err(err)?.migrated[0] != Some(1998)
        || people(&initial) != people(&final_view)
        || cash(&initial) != cash(&final_view)
        || r.interaction_time != 30
        || r.city(1).map_err(err)?.development_elapsed != 3153600000
        || r.city(2).map_err(err)?.population != 2098
        || r.obligations[0].ok_or("obligation")?.arrears.value() != 600000
    {
        return Err("regional_probe_invariant_failed".into());
    }
    Ok(
        json!({"initial":initial,"final":final_view,"candidates_from_city_1":candidates,
        "outcomes":outcomes,"replay":retry,"committed_sequence":restored.committed_sequence(),
        "state_sha256":before,"engine_owned_capacity_bytes":engine.owned_capacity_bytes(),
        "world_layout":engine.world().layout(),"region_inline_bytes":std::mem::size_of::<mycity_core::region::Region>(),
        "population_including_transit":people(&final_view),"cash_cents":cash(&final_view),
        "scope":"synthetic_anonymous_migration_fixed_second_obligations_and_atomic_recovery_only",
        "market_clearing":"not_implemented","electric_water_solver":"not_implemented",
        "full_scheduler_checkpoint":"not_tested","power_loss":"not_tested","memory_16gb_gate":"not_run"}),
    )
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(2);
    }
}

fn spatial_store_probe(
    opening: &[u8],
    input: &[u8],
    png: &[u8],
    dir: &std::path::Path,
) -> Result<serde_json::Value, String> {
    use mycity_core::{
        transaction::{Action, Caller, Envelope},
        units::Money,
    };
    use mycity_storage::Store;
    fn err(e: impl std::fmt::Debug) -> String {
        format!("{e:?}")
    }
    let mut store = Store::create_spatial(
        dir,
        opening,
        vec![],
        env!("MYCITY_SOURCE_SHA256"),
        input,
        png,
    )
    .map_err(err)?;
    let initial = store.engine().map_err(err)?.world().view();
    let initial_cash = initial.accounts.iter().map(|a| a.cash.value()).sum::<i64>();
    let reference = serde_json::to_value(store.spatial_reference().map_err(err)?).map_err(err)?;
    let cmd = Envelope {
        world: initial.scope.world(),
        branch: initial.scope.branch(),
        operation_id: 1,
        expected_revision: 0,
        actions: vec![Action::Transfer {
            from: store.engine().map_err(err)?.world().account_ref(2)?,
            to: store.engine().map_err(err)?.world().account_ref(3)?,
            amount: Money::new(10000).map_err(err)?,
        }],
    };
    let first = store
        .execute(Caller::Account(2), cmd.clone())
        .map_err(err)?;
    first.model.map_err(err)?;
    let before = store.state_hash().map_err(err)?;
    let owned = store.owned_capacity_bytes().map_err(err)?;
    let engine_owned = store.engine().map_err(err)?.owned_capacity_bytes();
    let spatial_owned = store
        .spatial()
        .map_err(err)?
        .ok_or("missing_spatial")?
        .owned_capacity_bytes();
    drop(store);
    let mut restored = Store::open(dir, env!("MYCITY_SOURCE_SHA256")).map_err(err)?;
    let retry = restored.execute(Caller::Account(2), cmd).map_err(err)?;
    let view = restored.engine().map_err(err)?.world().view();
    let assets = restored
        .spatial()
        .map_err(err)?
        .ok_or("missing_restored_spatial")?;
    if !first.has_persistent_receipt
        || !retry.has_persistent_receipt
        || before != restored.state_hash().map_err(err)?
        || restored.committed_sequence() != 1
        || reference
            != serde_json::to_value(restored.spatial_reference().map_err(err)?).map_err(err)?
        || initial_cash != view.accounts.iter().map(|a| a.cash.value()).sum::<i64>()
    {
        return Err("spatial_storage_invariant_failed".into());
    }
    Ok(
        json!({"schema":"mycity-m0-spatial-storage-evidence-v1","result":"joint_state_restored_once","spatial_reference":reference,
        "world":view,"committed_sequence":restored.committed_sequence(),"store_state_sha256":before,"first":first,"retry":retry,
        "store_owned_capacity_bytes":owned,"engine_owned_capacity_bytes":engine_owned,"spatial_owned_capacity_bytes":spatial_owned,
        "terrain_sha256":assets.terrain().state_hash(),"geometry_sha256":assets.scene().state_hash(),"terrain_vertices":assets.terrain().heights().len(),
        "allocator_metadata_and_replay_working_pages_included":false,"space":"immutable_genesis_assets_only_not_terrain_edits",
        "gpu":"not_run","economic_prototypes":"not_run","power_loss":"not_tested","memory_16gb_gate":"not_run"}),
    )
}
