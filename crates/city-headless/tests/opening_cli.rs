//! Actual selected files -> CLI -> single Store -> independent reader process.
//! Keep every input, world and failed command under the explicitly supplied evidence root.
use mycity_core::{
    sha256,
    transaction::{Action, Caller, Envelope},
    units::Money,
};
use mycity_storage::Store;
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn root(name: &str) -> PathBuf {
    let base = PathBuf::from(
        std::env::var_os("MYCITY_OPENING_CLI_TEST_ROOT")
            .expect("set dedicated HP P900 evidence root"),
    );
    let base = base.canonicalize().unwrap();
    assert!(base.starts_with("/Volumes/HP P900/dev/MyCity/work"));
    let p = base.join(format!("{name}-{}", std::process::id()));
    fs::create_dir(&p).unwrap();
    p
}
fn exe() -> PathBuf {
    std::env::var_os("MYCITY_PROTOCOL_WORKER_EXE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_mycity-headless")))
}
fn invoke(root: &Path, name: &str, args: &[&Path], success: bool) -> Value {
    let mut c = Command::new(exe())
        .arg("init-opening")
        .args(args)
        .stdin(Stdio::null())
        .stdout(File::create(root.join(format!("{name}.stdout"))).unwrap())
        .stderr(File::create(root.join(format!("{name}.stderr"))).unwrap())
        .spawn()
        .unwrap();
    let started = Instant::now();
    let status = loop {
        if let Some(status) = c.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > Duration::from_secs(30) {
            c.kill().unwrap();
            c.wait().unwrap();
            panic!("bounded CLI timeout");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    fs::write(
        root.join(format!("{name}.status.json")),
        serde_json::to_vec(&json!({"pid":c.id(),"exit":status.code()})).unwrap(),
    )
    .unwrap();
    assert_eq!(
        status.success(),
        success,
        "{}",
        fs::read_to_string(root.join(format!("{name}.stderr"))).unwrap()
    );
    let out = fs::read(root.join(format!("{name}.stdout"))).unwrap();
    assert!(out.len() < 64 * 1024);
    if success {
        serde_json::from_slice(&out).unwrap()
    } else {
        Value::Null
    }
}
const E0: &[u8] = include_bytes!("../../city-storage/tests/fixtures/opening-input-store/e0.json");
const TOWN: &[u8] =
    include_bytes!("../../city-storage/tests/fixtures/opening-input-store/town.json");
const SPATIAL: &[u8] =
    include_bytes!("../../city-storage/tests/fixtures/opening-input-store/spatial.json");
const PNG: &[u8] =
    include_bytes!("../../city-storage/tests/fixtures/opening-input-store/flat16.png");
#[test]
fn selected_real_files_publish_once_and_spent_capital_survives_fresh_reader() {
    let p = root("e0");
    let opening = p.join("opening.json");
    let resources = p.join("resources.json");
    let world = p.join("world");
    let mut raw = b" \n".to_vec();
    raw.extend_from_slice(E0);
    raw.extend_from_slice(b"\n ");
    fs::write(&opening, &raw).unwrap();
    fs::write(
        &resources,
        br#"{"schema":"mycity-resources-v1","resources":[{"id":501,"capacity":7}]}"#,
    )
    .unwrap();
    let report = invoke(&p, "create", &[&world, &opening, &resources], true);
    assert_eq!(report["opening_source"]["sha256"], sha256(&raw));
    assert_eq!(report["opening_source"]["bytes_read"], raw.len());
    assert_eq!(
        report["resource_source"]["source_label"],
        "local_file_os_read"
    );
    let mut store = Store::open(&world, env!("MYCITY_SOURCE_SHA256")).unwrap();
    assert_eq!(store.state_hash().unwrap(), report["state_sha256"]);
    let view = serde_json::to_value(store.engine().unwrap().world().view()).unwrap();
    assert_eq!(view["accounts"][0]["cash"], 1000000);
    assert_eq!(view["stocks"][0]["owner"], 4);
    assert_eq!(view["stocks"][0]["quantity"], 600000);
    let resource = &store.engine().unwrap().resources()[0];
    assert_eq!(
        (
            resource.id,
            resource.capacity,
            resource.reserved,
            resource.occupied
        ),
        (501, 7, 0, 0)
    );
    let revision = store.engine().unwrap().world().scope_revision().1;
    let from = store.engine().unwrap().world().account_ref(1).unwrap();
    let to = store.engine().unwrap().world().account_ref(4).unwrap();
    store
        .execute(
            Caller::Account(1),
            Envelope {
                world: 1,
                branch: 1,
                operation_id: 77,
                expected_revision: revision,
                actions: vec![Action::Transfer {
                    from,
                    to,
                    amount: Money::new(137).unwrap(),
                }],
            },
        )
        .unwrap()
        .model
        .unwrap();
    let spent = store.state_hash().unwrap();
    drop(store);
    invoke(
        &p,
        "reject-recreate",
        &[&world, &opening, &resources],
        false,
    );
    let restored = Store::open(&world, env!("MYCITY_SOURCE_SHA256")).unwrap();
    assert_eq!(restored.state_hash().unwrap(), spent);
    let view = serde_json::to_value(restored.engine().unwrap().world().view()).unwrap();
    assert_eq!(view["accounts"][0]["cash"], 999863);
    fs::write(
        p.join("restored-view.json"),
        serde_json::to_vec_pretty(&view).unwrap(),
    )
    .unwrap();
}
#[test]
fn missing_spatial_and_bad_sources_never_bypass_canonical_input_gates() {
    let p = root("town");
    let opening = p.join("opening.json");
    let resources = p.join("resources.json");
    let spatial = p.join("spatial.json");
    let png = p.join("height.png");
    fs::write(&opening, TOWN).unwrap();
    fs::write(&spatial, SPATIAL).unwrap();
    fs::write(&png, PNG).unwrap();
    let entries: Vec<_> = [
        (101, 1),
        (102, 1),
        (103, 1),
        (104, 1),
        (201, 2000),
        (202, 2000),
    ]
    .into_iter()
    .map(|(id, capacity)| json!({"id":id,"capacity":capacity}))
    .collect();
    fs::write(
        &resources,
        serde_json::to_vec(&json!({"schema":"mycity-resources-v1","resources":entries})).unwrap(),
    )
    .unwrap();
    let missing = p.join("missing-spatial");
    invoke(
        &p,
        "missing-spatial",
        &[&missing, &opening, &resources],
        false,
    );
    assert!(!missing.join("CURRENT.json").exists());
    let world = p.join("world");
    let report = invoke(
        &p,
        "town-create",
        &[&world, &opening, &resources, &spatial, &png],
        true,
    );
    let store = Store::open(&world, env!("MYCITY_SOURCE_SHA256")).unwrap();
    assert!(store.spatial().unwrap().is_some());
    assert_eq!(store.state_hash().unwrap(), report["state_sha256"]);
    drop(store);
    let bad = p.join("bad-resources.json");
    fs::write(
        &bad,
        br#"{"schema":"mycity-resources-v1","resources":[{"id":101,"capacity":2,"occupied":1}]}"#,
    )
    .unwrap();
    let badworld = p.join("bad-resource-world");
    invoke(&p, "bad-resource", &[&badworld, &opening, &bad], false);
    assert!(!badworld.exists());
    let invalid = p.join("invalid.json");
    fs::write(&invalid, b"{").unwrap();
    let invalidworld = p.join("invalid-world");
    invoke(
        &p,
        "invalid-opening",
        &[&invalidworld, &invalid, &resources],
        false,
    );
    assert!(!invalidworld.join("CURRENT.json").exists());
}
