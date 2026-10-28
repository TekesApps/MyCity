//! #158 explicit local pool cost version; V1 recovery retains its original facts.
use mycity_core::{boot::{Command as B, Facility}, housing,
    service_delivery_authority::Command as D, timeline::Command as T,
    transaction::{Action, Caller, Envelope}};
use mycity_storage::Store;
use serde_json::{json, Value};
use std::{fs, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};
#[path="../../../tests/fixtures/finite-city/opening.rs"] mod fixture;
const RULES: &str = "finite-city-boot-partition-v1";
fn apply(s: &mut Store, caller: Caller, action: Action) -> Envelope {
    let (scope, revision) = s.engine().unwrap().world().scope_revision();
    let ticket = Envelope { world:scope.world(), branch:scope.branch(),
        operation_id:s.committed_sequence()+1, expected_revision:revision, actions:vec![action] };
    let result = s.execute(caller, ticket.clone()).unwrap();
    assert!(result.model.is_ok(), "{result:?}");
    ticket
}
fn advance(s: &mut Store, target: u64) -> Envelope {
    for _ in 0..8192 {
        let limit = s.continuation_status().unwrap().map_or(256, |r|r.page_limit);
        let ticket = apply(s, Caller::System, Action::Timeline { command:T::Advance { target, limit } });
        let progress = s.engine().unwrap().timeline().unwrap().progress;
        if progress.target.is_none() { assert_eq!(progress.last_complete, target); return ticket; }
    }
    panic!("unfinished original target {target}");
}
fn run(root: &Path, label: &str, points: &[u64], exact: bool, reopen: bool) -> Value {
    let p = root.join(label); fs::create_dir(&p).unwrap();
    let mut raw = fixture::boot_service_input();
    if exact { raw["boot"]["profile"] = json!(mycity_core::boot::LOCAL_EXACT_PROFILE); }
    let bytes = serde_json::to_vec(&raw).unwrap();
    fs::write(p.join("opening.json"), &bytes).unwrap();
    let mut s = Store::create(&p.join("world"), &bytes, fixture::resources(), RULES).unwrap();
    apply(&mut s, Caller::System, Action::Timeline { command:T::Enable });
    let offer:housing::GenesisLeaseOffer=serde_json::from_value(raw["housing"]["lease_offers"][0].clone()).unwrap();
    apply(&mut s, Caller::Account(2), Action::HousingLease { offer_id:901, offer_sha256:housing::genesis_lease_offer_sha256(offer) });
    for (facility, until) in [(Facility::Pump,60),(Facility::Generator,120)] {
        apply(&mut s, Caller::System, Action::Boot { command:B::Start { facility, reverse_request:false } });
        advance(&mut s, until);
    }
    advance(&mut s, 28800);
    apply(&mut s, Caller::Account(3), Action::ServiceDelivery { command:D::Accept { household:7, crew:1 } });
    let mut last = None;
    for (i, &point) in points.iter().enumerate() {
        last = Some(advance(&mut s, point));
        if reopen && (i == 1 || i == 2) {
            // First cold open replays WAL. The second restores the published checkpoint.
            let checkpoint = s.engine().unwrap().export_checkpoint().unwrap();
            let file = if i == 1 { "mid-wal.json" } else { "mid-checkpoint.json" };
            fs::write(p.join(file), &checkpoint).unwrap();
            if i == 2 { s.checkpoint().unwrap(); }
            drop(s);
            cold_check(&p, file);
            s = Store::open(&p.join("world"), RULES).unwrap();
            assert_eq!(s.engine().unwrap().export_checkpoint().unwrap(), checkpoint);
        }
    }
    let e=s.engine().unwrap();let v=e.world().view();let boot=v.boot.unwrap();
    let delivery=e.service_needs_summary().unwrap().delivery.unwrap();let trip=&delivery.trips[0];
    let checkpoint=e.export_checkpoint().unwrap();fs::write(p.join("complete.json"), &checkpoint).unwrap();
    let cp:Value=serde_json::from_slice(&checkpoint).unwrap();let use_=&cp["world"]["service_needs"]["delivery"]["boot_use"];
    let out=json!({
        "quantities":v.stocks.iter().map(|x|(x.id,x.quantity.value())).collect::<Vec<_>>(),
        "cash":v.accounts.iter().map(|x|(x.id,x.cash.value(),x.protected.value())).collect::<Vec<_>>(),
        "trip":{"motion":trip.motion,"wages_paid":trip.wages_paid,"fee_paid":trip.fee_paid,
            "drive_energy_mwh":trip.drive_energy_mwh,"work_seconds":delivery.tasks[0].work_seconds},
        "usage_quantity":use_["quantity"],"usage_basis":use_["basis"],
        "stock_basis":v.stocks.iter().map(|x|(x.id,x.basis.value())).collect::<Vec<_>>(),
        "export_basis":boot.exported_basis,"exports":boot.exports,
        "all_boot_basis":v.stocks.iter().filter(|x|boot.stocks.contains(&x.id)).map(|x|x.basis.value()).sum::<i64>()+boot.exported_basis.value()+boot.failed_expense.value(),
    });
    fs::write(p.join("business.json"), serde_json::to_vec_pretty(&out).unwrap()).unwrap();
    // Duplicate the original last operation: no new cost or sequence.
    let replay = last.unwrap();
    let hash = s.state_hash().unwrap(); let sequence = s.committed_sequence();
    assert!(s.execute(Caller::System, replay).unwrap().model.is_ok());
    assert_eq!(s.committed_sequence(), sequence); assert_eq!(s.state_hash().unwrap(), hash);
    let checkpoint = s.engine().unwrap().export_checkpoint().unwrap();
    fs::write(p.join("complete.json"), &checkpoint).unwrap();
    s.checkpoint().unwrap();drop(s);
    cold_check(&p, "complete.json");
    assert_eq!(Store::open(&p.join("world"),RULES).unwrap().engine().unwrap().export_checkpoint().unwrap(),checkpoint);
    out
}
fn cold_check(p: &Path, expected: &str) {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "partition_cold_open", "--test-threads=1"])
        .env("MYCITY_BOOT_PARTITION_COLD", p).env("MYCITY_BOOT_PARTITION_EXPECTED", expected)
        .output().unwrap();
    fs::write(p.join(format!("{expected}-cold.stdout")), &output.stdout).unwrap();
    fs::write(p.join(format!("{expected}-cold.stderr")), &output.stderr).unwrap();
    assert!(output.status.success(), "cold process: {:?}", output.status);
}
#[test] #[ignore]
fn partition_cold_open() {
    let p = PathBuf::from(std::env::var_os("MYCITY_BOOT_PARTITION_COLD").unwrap());
    let expected = fs::read(p.join(std::env::var_os("MYCITY_BOOT_PARTITION_EXPECTED").unwrap())).unwrap();
    let s = Store::open(&p.join("world"), RULES).unwrap();
    assert_eq!(s.engine().unwrap().export_checkpoint().unwrap(), expected);
}
fn compare(exact: bool) {
    let root=PathBuf::from(std::env::var_os("MYCITY_BOOT_PARTITION_ROOT").unwrap()).canonicalize().unwrap();
    assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"));
    let p=root.join(format!("same-trip-{}-{}-{}",if exact {"v2"} else {"v1"},std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));fs::create_dir(&p).unwrap();
    let direct=run(&p,"direct",&[33900],exact,false);
    let paused=run(&p,"paused",&[29130,29160,29190,33900],exact,true);
    for key in ["quantities","cash","trip","usage_quantity","exports","all_boot_basis"] {
        assert_eq!(direct[key],paused[key],"same actual {key}");
    }
    assert_eq!(direct["all_boot_basis"],json!(2600));
    fs::write(p.join("comparison.json"),serde_json::to_vec_pretty(&json!({"direct":direct,"paused":paused})).unwrap()).unwrap();
    if exact {
        for key in ["stock_basis", "usage_basis", "export_basis"] { assert_eq!(direct[key], paused[key], "{key}"); }
        // Original electrical pool 400000/730: floor(730*130000/400000)=237.
        assert_eq!(direct["usage_basis"],json!([237,5]));
        assert_eq!(direct["export_basis"],json!(242));
        assert!(direct["stock_basis"].as_array().unwrap().contains(&json!([3002,493])));
    } else {
        // Compatibility deliberately retains V1's historical rounding, not equivalence.
        assert_eq!(direct["usage_basis"],json!([235,5]));
        assert_eq!(paused["usage_basis"],json!([234,5]));
        assert_eq!(direct["export_basis"],json!(240));
        assert_eq!(paused["export_basis"],json!(239));
    }
}
#[test] fn exact_local_pool_cost_is_invariant_under_pause_and_cold_resume() { compare(true); }
#[test] fn legacy_local_cost_replay_keeps_its_original_rounding() { compare(false); }

#[test]
fn legacy_v224_store_restores_the_original_cost_facts() {
    // Copies of the real red probe's worlds, never newly reconstructed history.
    let p = PathBuf::from(std::env::var_os("MYCITY_BOOT_PARTITION_LEGACY").unwrap());
    for (label, expected) in [("direct",235), ("paused",234)] {
        let branch = p.join(label);
        let checkpoint = fs::read(branch.join("complete.json")).unwrap();
        let s = Store::open(&branch.join("world"), RULES).unwrap();
        assert_eq!(s.engine().unwrap().export_checkpoint().unwrap(), checkpoint);
        let cp:Value = serde_json::from_slice(&checkpoint).unwrap();
        assert_eq!(cp["world"]["boot"]["profile"],json!(mycity_core::boot::LOCAL_PROFILE));
        assert_eq!(cp["world"]["service_needs"]["delivery"]["boot_use"]["basis"],json!([expected,5]));
    }
}
#[test]
fn unsupported_local_cost_version_is_not_silently_mapped() {
    let root=PathBuf::from(std::env::var_os("MYCITY_BOOT_PARTITION_ROOT").unwrap()).canonicalize().unwrap();
    let p=root.join(format!("unsupported-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
    let mut raw=fixture::boot_service_input();raw["boot"]["profile"]=json!("pboot-local-development-v99");
    assert!(Store::create(&p,&serde_json::to_vec(&raw).unwrap(),fixture::resources(),RULES).is_err());
}
