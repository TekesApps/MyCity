use mycity_core::{
    sha256,
    transaction::{Action, Caller, Envelope, Resource},
    units::Money,
};
use mycity_storage::{CreatePoint, Store, WritePoint};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
const OPENING: &[u8] = include_bytes!("../../../fixtures/m0-region-opening.json");
const SPATIAL: &[u8] = include_bytes!("../../../fixtures/m0-spatial.json");
const PNG: &[u8] = include_bytes!("../../../fixtures/terrain/m0-flat16.png");
const RULES: &str = "m0-storage-process-fixture-v1";
static ID: AtomicU64 = AtomicU64::new(1);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "mycity-spatial-store-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn world(&self) -> PathBuf {
        self.0.join("world")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn create(p: &Path) -> Store {
    Store::create_spatial(
        p,
        OPENING,
        vec![Resource::new(1, 10).unwrap()],
        RULES,
        SPATIAL,
        PNG,
    )
    .unwrap()
}
fn inspect(p: &str) -> std::process::Output {
    worker(&["inspect", p])
}
fn worker(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mycity-storage-worker"))
        .args(args)
        .output()
        .unwrap()
}
fn joint_command(s: &Store) -> Envelope {
    Envelope {
        world: 1,
        branch: 1,
        operation_id: 1,
        expected_revision: 0,
        actions: vec![
            Action::Transfer {
                from: s.engine().unwrap().world().account_ref(2).unwrap(),
                to: s.engine().unwrap().world().account_ref(3).unwrap(),
                amount: Money::new(10000).unwrap(),
            },
            Action::RegionMigrate {
                from: 1,
                to: 2,
                requested: 2000,
                tolerance: 2,
            },
            Action::RegionAdvance { seconds: 30 },
            Action::RegionActivate { city: 1 },
        ],
    }
}
fn rewrite_genesis(p: &Path, change: impl FnOnce(&mut Value)) {
    // Test a self-consistent root's semantic gates, not only the outer checksums.
    let mut g: Value = serde_json::from_slice(&fs::read(p.join("genesis.json")).unwrap()).unwrap();
    change(&mut g);
    let gb = serde_json::to_vec(&g).unwrap();
    fs::write(p.join("genesis.json"), &gb).unwrap();
    let mut r: Value = serde_json::from_slice(&fs::read(p.join("root-0.json")).unwrap()).unwrap();
    assert_eq!(r["sequence"], 0);
    r["genesis_sha256"] = json!(sha256(&gb));
    r["wal_chain"] = json!(sha256(&gb));
    let rb = serde_json::to_vec(&r).unwrap();
    fs::write(p.join("root-0.json"), &rb).unwrap();
    let mut c: Value = serde_json::from_slice(&fs::read(p.join("CURRENT.json")).unwrap()).unwrap();
    c["root_sha256"] = json!(sha256(&rb));
    fs::write(p.join("CURRENT.json"), serde_json::to_vec(&c).unwrap()).unwrap();
}
#[test]
fn spatial_financial_and_regional_state_restore_under_one_root_without_double_capital_or_payment() {
    let t = Temp::new();
    let p = t.world();
    let mut s = create(&p);
    let original_ref = serde_json::to_value(s.spatial_reference().unwrap()).unwrap();
    let original_model = s.engine().unwrap().state_hash();
    let original_joint = s.state_hash().unwrap();
    let cmd = joint_command(&s);
    let result = s.execute(Caller::System, cmd.clone()).unwrap();
    assert!(result.has_persistent_receipt);
    result.model.unwrap();
    let before = s.state_hash().unwrap();
    assert_ne!(original_joint, before);
    assert_ne!(original_model, s.engine().unwrap().state_hash());
    assert!(
        s.owned_capacity_bytes().unwrap()
            > s.engine().unwrap().owned_capacity_bytes()
                + s.spatial().unwrap().unwrap().owned_capacity_bytes()
    );
    drop(s);
    let mut s = Store::open(&p, RULES).unwrap();
    assert_eq!(s.state_hash().unwrap(), before);
    assert_eq!(
        serde_json::to_value(s.spatial_reference().unwrap()).unwrap(),
        original_ref
    );
    assert!(
        s.execute(Caller::System, cmd)
            .unwrap()
            .has_persistent_receipt
    );
    assert_eq!(s.committed_sequence(), 1);
    assert_eq!(s.state_hash().unwrap(), before);
    let v = s.engine().unwrap().world().view();
    assert_eq!(
        v.accounts.iter().map(|a| a.cash.value()).sum::<i64>(),
        3000000
    );
    let r = v.region.unwrap();
    assert_eq!(r.interaction_time, 30);
    assert_eq!(r.city(2).unwrap().population, 2098);
    assert_eq!(r.obligations[0].unwrap().arrears.value(), 590000);
    assert_eq!(
        s.spatial().unwrap().unwrap().terrain().heights().len(),
        66049
    );
}
#[test]
fn invalid_sources_and_cross_world_branch_or_seed_fail_before_any_directory_is_created() {
    for (key, value) in [("world", 2), ("branch", 2), ("seed", 8)] {
        let t = Temp::new();
        let mut input: Value = serde_json::from_slice(SPATIAL).unwrap();
        input["bootstrap"][key] = json!(value);
        assert!(Store::create_spatial(
            &t.world(),
            OPENING,
            vec![],
            RULES,
            &serde_json::to_vec(&input).unwrap(),
            PNG
        )
        .is_err());
        assert!(!t.world().exists());
    }
    let t = Temp::new();
    assert!(
        Store::create_spatial(&t.world(), OPENING, vec![], RULES, SPATIAL, b"not PNG").is_err()
    );
    assert!(!t.world().exists());
    fs::create_dir(t.world()).unwrap();
    fs::write(t.world().join("keep-user-data"), b"retain").unwrap();
    assert!(Store::create_spatial(&t.world(), OPENING, vec![], RULES, SPATIAL, PNG).is_err());
    assert_eq!(
        fs::read(t.world().join("keep-user-data")).unwrap(),
        b"retain"
    );
}
#[test]
fn v1_v2_schema_and_derived_hash_gates_do_not_silently_drop_space() {
    for defect in [
        "v1_with_space",
        "v2_without_space",
        "terrain_hash",
        "geometry_hash",
        "invalid_byte_limit",
        "unknown_field",
    ] {
        let t = Temp::new();
        let p = t.world();
        drop(create(&p));
        rewrite_genesis(&p, |g| match defect {
            "v1_with_space" => g["schema"] = json!("mycity-m0-genesis-v1"),
            "v2_without_space" => {
                g.as_object_mut().unwrap().remove("spatial");
            }
            "terrain_hash" => g["spatial"]["terrain_sha256"] = json!("0".repeat(64)),
            "geometry_hash" => g["spatial"]["geometry_sha256"] = json!("0".repeat(64)),
            "invalid_byte_limit" => g["spatial"]["png_bytes"] = json!(u64::MAX),
            _ => g["spatial"]["png_file"] = json!("../../outside"),
        });
        let root = fs::read(p.join("CURRENT.json")).unwrap();
        assert!(Store::open(&p, RULES).is_err(), "{defect}");
        assert_eq!(fs::read(p.join("CURRENT.json")).unwrap(), root);
    }
    let t = Temp::new();
    drop(Store::create(&t.world(), OPENING, vec![], RULES).unwrap());
    let plain = Store::open(&t.world(), RULES).unwrap();
    assert!(plain.spatial().unwrap().is_none());
}
#[test]
fn corrupt_missing_nonregular_or_symlink_space_never_opens_or_truncates_wal_tail() {
    use std::io::Write;
    for defect in [
        "png_corrupt",
        "json_corrupt",
        "png_missing",
        "json_missing",
        "png_directory",
        "png_symlink",
    ] {
        let t = Temp::new();
        let p = t.world();
        let mut s = create(&p);
        let cmd = joint_command(&s);
        s.execute(Caller::System, cmd).unwrap().model.unwrap();
        drop(s);
        let current = fs::read(p.join("CURRENT.json")).unwrap();
        let genesis = fs::read(p.join("genesis.json")).unwrap();
        fs::OpenOptions::new()
            .append(true)
            .open(p.join("wal.bin"))
            .unwrap()
            .write_all(b"uncommitted-tail-retain-on-invalid-dependency")
            .unwrap();
        let wal = fs::read(p.join("wal.bin")).unwrap();
        match defect {
            "png_corrupt" => {
                let mut v = PNG.to_vec();
                v[42] ^= 1;
                fs::write(p.join("spatial-height.png"), v).unwrap();
            }
            "json_corrupt" => fs::write(p.join("spatial-input.json"), b"{}").unwrap(),
            "png_missing" => fs::remove_file(p.join("spatial-height.png")).unwrap(),
            "json_missing" => fs::remove_file(p.join("spatial-input.json")).unwrap(),
            "png_directory" => {
                fs::remove_file(p.join("spatial-height.png")).unwrap();
                fs::create_dir(p.join("spatial-height.png")).unwrap();
            }
            _ => {
                fs::remove_file(p.join("spatial-height.png")).unwrap();
                std::os::unix::fs::symlink(p.join("genesis.json"), p.join("spatial-height.png"))
                    .unwrap();
            }
        }
        assert!(Store::open(&p, RULES).is_err(), "{defect}");
        assert_eq!(fs::read(p.join("CURRENT.json")).unwrap(), current);
        assert_eq!(fs::read(p.join("genesis.json")).unwrap(), genesis);
        assert_eq!(fs::read(p.join("wal.bin")).unwrap(), wal);
    }
}
#[test]
fn matching_file_checksums_cannot_hide_wrong_identity_or_terrain_interpretation() {
    for defect in ["seed", "height_rule"] {
        let t = Temp::new();
        let p = t.world();
        drop(create(&p));
        let mut input: Value = serde_json::from_slice(SPATIAL).unwrap();
        if defect == "seed" {
            input["bootstrap"]["seed"] = json!(8);
        } else {
            input["terrain"]["z_max_cm"] = json!(65534);
        }
        let data = serde_json::to_vec(&input).unwrap();
        fs::write(p.join("spatial-input.json"), &data).unwrap();
        rewrite_genesis(&p, |g| {
            g["spatial"]["input_bytes"] = json!(data.len());
            g["spatial"]["input_sha256"] = json!(sha256(&data));
        });
        assert!(Store::open(&p, RULES).is_err(), "{defect}");
    }
}
#[test]
fn create_io_failure_before_publication_retains_incomplete_directory_and_after_publication_requires_reopen(
) {
    for (point, committed) in [
        (CreatePoint::SpatialPngHalf, false),
        (CreatePoint::CurrentRenamed, true),
        (CreatePoint::DirectorySynced, true),
    ] {
        let t = Temp::new();
        let p = t.world();
        assert!(Store::create_spatial_with_observer(
            &p,
            OPENING,
            vec![],
            RULES,
            (SPATIAL, PNG),
            |at| if at == point {
                Err(std::io::Error::from_raw_os_error(28))
            } else {
                Ok(())
            }
        )
        .is_err());
        assert!(p.exists());
        assert!(Store::create_spatial(&p, OPENING, vec![], RULES, SPATIAL, PNG).is_err());
        let restored = Store::open(&p, RULES);
        assert_eq!(restored.is_ok(), committed, "{point:?}");
        if let Ok(s) = restored {
            assert_eq!(
                s.engine()
                    .unwrap()
                    .world()
                    .view()
                    .accounts
                    .iter()
                    .map(|a| a.cash.value())
                    .sum::<i64>(),
                3000000
            );
            assert!(s.spatial().unwrap().is_some());
        }
    }
}
#[test]
fn uncertain_transaction_hides_both_model_and_space_until_joint_recovery() {
    let t = Temp::new();
    let p = t.world();
    let mut s = create(&p);
    let cmd = joint_command(&s);
    assert!(s
        .execute_with_observer(Caller::System, cmd.clone(), |at| {
            if at == WritePoint::CurrentRenamed {
                Err(std::io::Error::from_raw_os_error(28))
            } else {
                Ok(())
            }
        })
        .is_err());
    assert!(s.engine().is_err());
    assert!(s.spatial().is_err());
    assert!(s.spatial_reference().is_err());
    assert!(s.state_hash().is_err());
    drop(s);
    let mut s = Store::open(&p, RULES).unwrap();
    assert_eq!(s.committed_sequence(), 1);
    let before = s.state_hash().unwrap();
    assert!(
        s.execute(Caller::System, cmd)
            .unwrap()
            .has_persistent_receipt
    );
    assert_eq!(s.state_hash().unwrap(), before);
}
#[test]
fn abrupt_creation_exit_publishes_all_dependencies_and_capital_or_no_world() {
    for (point, committed) in [
        ("Validated", false),
        ("SpatialInputHalf", false),
        ("SpatialInputSynced", false),
        ("SpatialPngHalf", false),
        ("SpatialPngSynced", false),
        ("GenesisSynced", false),
        ("WalSynced", false),
        ("RootSynced", false),
        ("CurrentRenamed", true),
        ("CurrentSynced", true),
        ("DirectorySynced", true),
        ("Ready", true),
        ("Responded", true),
    ] {
        let t = Temp::new();
        let p = t.world();
        let p = p.to_str().unwrap();
        let killed = worker(&["init-spatial", p, point]);
        assert_eq!(
            killed.status.code(),
            Some(97),
            "{point}: {}",
            String::from_utf8_lossy(&killed.stderr)
        );
        let out = inspect(p);
        assert_eq!(
            out.status.success(),
            committed,
            "{point}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        if committed {
            let v: Value = serde_json::from_slice(&out.stdout).unwrap();
            assert_eq!(v["sequence"], 0);
            assert_eq!(v["world"]["revision"], 0);
            assert_eq!(v["world"]["accounts"][2]["cash"], 600000);
            assert!(v["spatial_reference"].is_object());
            assert!(!worker(&["init-spatial", p, "none"]).status.success());
            let after = inspect(p);
            assert_eq!(serde_json::from_slice::<Value>(&after.stdout).unwrap(), v);
        }
    }
}
#[test]
fn abrupt_transaction_exit_preserves_space_with_the_correct_financial_and_regional_root() {
    for (point, committed) in [
        ("Prepared", false),
        ("WalHeaderHalf", false),
        ("WalPayloadHalf", false),
        ("WalChecksumHalf", false),
        ("WalWritten", false),
        ("WalSynced", false),
        ("RootPublished", false),
        ("CurrentRenamed", true),
        ("CurrentSynced", true),
        ("Applied", true),
        ("Responded", true),
    ] {
        let t = Temp::new();
        let p = t.world();
        let p = p.to_str().unwrap();
        let init = worker(&["init-spatial", p, "none"]);
        assert!(init.status.success());
        let out = inspect(p);
        let original: Value = serde_json::from_slice(&out.stdout).unwrap();
        let killed = worker(&["commit", p, point]);
        assert_eq!(killed.status.code(), Some(97));
        let out = inspect(p);
        assert!(
            out.status.success(),
            "{point}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["spatial_reference"], original["spatial_reference"]);
        assert_eq!(v["sequence"], u64::from(committed));
        assert_eq!(
            v["world"]["region"]["interaction_time"],
            if committed { 30 } else { 0 }
        );
        assert_eq!(
            v["world"]["region"]["cities"][1]["population"],
            if committed { 2098 } else { 100 }
        );
        assert_eq!(
            v["world"]["accounts"][2]["cash"],
            if committed { 0 } else { 600000 }
        );
        if committed {
            assert!(worker(&["commit", p, "none"]).status.success());
            let out = inspect(p);
            assert_eq!(serde_json::from_slice::<Value>(&out.stdout).unwrap(), v);
        }
    }
}
