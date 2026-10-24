//! Root-owned Store entrypoints are required; this exclusive source does not export or
//! initialize a parallel model. All artifacts survive success, failure and panic.
use mycity_core::{
    canonical::Codec,
    opening_loader::{InputOrigin, OpeningInput},
    transaction::{Action, Caller, EngineWorkspace, Envelope, Resource},
    units::Money,
};
use mycity_storage::{CreatePoint, Error, Store};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const APPROVED_BASE: &str = "/Volumes/HP P900/dev/MyCity/work";
const ROOT_ENV: &str = "MYCITY_OPENING_INPUT_STORE_ROOT";
const PROBE_ENV: &str = "MYCITY_OPENING_INPUT_STORE_PROBE";
const RULES: &str = "issue120-opening-input-store-v1";
const RUN_BYTES_LIMIT: u64 = 64 * 1024 * 1024;
const E0: &[u8] = include_bytes!("fixtures/opening-input-store/e0.json");
const BOOT: &[u8] = include_bytes!("fixtures/opening-input-store/boot.json");
const TOWN: &[u8] = include_bytes!("fixtures/opening-input-store/town.json");
const SPATIAL: &[u8] = include_bytes!("fixtures/opening-input-store/spatial.json");
const PNG: &[u8] = include_bytes!("fixtures/opening-input-store/flat16.png");
const LEDGERS: &str = include_str!("fixtures/opening-input-store/expected-ledgers.json");
static NEXT: AtomicU64 = AtomicU64::new(0);

fn digest(bytes: &[u8]) -> String {
    // Independent envelope/raw-file calculation; do not use Store or loader hash helpers.
    format!("{:x}", Sha256::digest(bytes))
}

fn configured_root() -> PathBuf {
    let approved = fs::canonicalize(APPROVED_BASE).expect("existing HP P900 work mount required");
    assert_eq!(approved, Path::new(APPROVED_BASE), "approved base must not redirect elsewhere");
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_ne!(
            fs::metadata("/Volumes/HP P900").unwrap().dev(),
            fs::metadata("/Volumes").unwrap().dev(),
            "HP P900 must be a mounted filesystem, not a local fallback directory",
        );
    }
    let configured = PathBuf::from(std::env::var_os(ROOT_ENV).expect(
        "set MYCITY_OPENING_INPUT_STORE_ROOT to an existing unique run directory below HP P900 work",
    ));
    assert!(configured.is_absolute());
    let root = fs::canonicalize(configured).expect("configured run root must already exist");
    assert!(root.is_dir() && root != approved && root.starts_with(&approved));
    root
}

fn files(path: &Path) -> BTreeMap<String, (u64, String)> {
    fn visit(base: &Path, path: &Path, result: &mut BTreeMap<String, (u64, String)>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            assert!(!kind.is_symlink(), "retained artifact tree must not redirect outside its root");
            if kind.is_dir() {
                visit(base, &entry.path(), result);
            } else {
                assert!(kind.is_file());
                let mut file = fs::File::open(entry.path()).unwrap();
                let mut hasher = Sha256::new();
                let mut buffer = [0u8; 8192];
                let mut size = 0u64;
                loop {
                    let count = file.read(&mut buffer).unwrap();
                    if count == 0 { break; }
                    size = size.checked_add(count as u64).unwrap();
                    hasher.update(&buffer[..count]);
                }
                result.insert(
                    entry.path().strip_prefix(base).unwrap().to_string_lossy().into_owned(),
                    (size, format!("{:x}", hasher.finalize())),
                );
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(path, path, &mut result);
    result
}

fn assert_run_budget(root: &Path) {
    let total: u64 = files(root).values().map(|(bytes, _)| *bytes).sum();
    assert!(total <= RUN_BYTES_LIMIT, "retained run exceeds 64MiB: {total}; retain all evidence");
}

fn retain(path: &Path, bytes: &[u8]) {
    let mut file = OpenOptions::new().create_new(true).write(true).open(path).unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
}

struct Artifacts { root: PathBuf, case: PathBuf }
impl Artifacts {
    fn new(name: &str) -> Self {
        let root = configured_root();
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let serial = NEXT.fetch_add(1, Ordering::Relaxed);
        let case = root.join(format!("{name}-{}-{nonce}-{serial}", std::process::id()));
        fs::create_dir(&case).expect("reserve unique retained case; never reuse or remove");
        assert!(fs::canonicalize(&case).unwrap().starts_with(&root));
        eprintln!("opening_input_store retained artifacts: {}", case.display());
        let this = Self { root, case };
        this.record("case", &json!({"name":name,"pid":std::process::id(),"root":this.case}));
        this
    }
    fn world(&self, name: &str) -> PathBuf { self.case.join(name) }
    fn record(&self, name: &str, value: &Value) {
        retain(&self.case.join(format!("{name}.json")), &serde_json::to_vec_pretty(value).unwrap());
        assert_run_budget(&self.root);
    }
}
// Deliberately no Drop cleanup, including on panic. Incomplete worlds remain intact.

fn fixture(name: &str) -> &'static [u8] {
    match name { "e0" => E0, "boot" => BOOT, "town" => TOWN, _ => panic!("unknown fixture") }
}
fn source_bytes(name: &str) -> Vec<u8> {
    let mut bytes = b"\r\n  ".to_vec();
    bytes.extend_from_slice(fixture(name));
    bytes.extend_from_slice(b"\n\t ");
    bytes
}
fn input(name: &str) -> OpeningInput {
    let bytes = source_bytes(name);
    OpeningInput::read_named_json(bytes.as_slice(), &format!("fixture://issue120-store/{name}")).unwrap()
}
fn resources(name: &str) -> Vec<Resource> {
    let rows: &[(u32, u32)] = match name {
        "e0" => &[],
        "boot" => &[(101, 2), (102, 2)],
        "town" => &[(101, 1), (102, 1), (103, 1), (104, 1), (201, 2000), (202, 2000)],
        _ => panic!("unknown fixture"),
    };
    rows.iter().map(|&(id, cap)| Resource::new(id, cap).unwrap()).collect()
}

fn observe(store: &Store) -> Value {
    let mut codec = Codec::new().unwrap();
    let mut bytes = Vec::new();
    store.engine().unwrap().write_state_comparison(&mut codec, &mut bytes).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn assert_ledger(store: &Store, name: &str, cash_delta: i64, sequence: u64) -> Value {
    let expected: Value = serde_json::from_str(LEDGERS).unwrap();
    let expected = &expected[name];
    let actual = observe(store);
    let world = &actual["world"];
    assert_eq!(store.committed_sequence(), sequence);
    assert_eq!(world["revision"].as_u64(), Some(sequence));
    let accounts = world["accounts"].as_array().unwrap();
    assert_eq!(accounts.len(), expected["accounts"].as_array().unwrap().len());
    for row in expected["accounts"].as_array().unwrap() {
        let id = row[0].as_u64().unwrap();
        let account = accounts.iter().find(|v| v["id"].as_u64() == Some(id)).unwrap();
        let change = match id { 1 => -cash_delta, 4 => cash_delta, _ => 0 };
        assert_eq!(account["role"], row[1]);
        assert_eq!(account["cash"].as_i64(), Some(row[2].as_i64().unwrap() + change));
    }
    let sources: Vec<Value> = world["opening_sources"].as_array().unwrap().iter()
        .map(|s| json!([s["id"],s["kind"],s["amount"]])).collect();
    assert_eq!(json!(sources), expected["sources"]);
    let stocks = world["stocks"].as_array().unwrap();
    assert_eq!(stocks.len(), expected["stocks"].as_array().unwrap().len());
    for row in expected["stocks"].as_array().unwrap() {
        let stock = stocks.iter().find(|v| v["id"] == row[0]).unwrap();
        assert_eq!(json!([stock["id"],stock["owner"],stock["commodity"],stock["quantity"],stock["basis"],stock["lot_id"]]), *row);
        assert_eq!(stock["origin_quantity"], row[3]);
        assert_eq!(stock["origin_basis"], row[4]);
        assert_eq!(stock["offset"], 0);
    }
    let actual_resources: Vec<_> = store.engine().unwrap().resources().iter()
        .map(|r| (r.id, r.capacity, r.reserved, r.occupied)).collect();
    let expected_resources: Vec<_> = resources(name).iter().map(|r| (r.id,r.capacity,0,0)).collect();
    assert_eq!(actual_resources, expected_resources);
    actual
}

fn assert_genesis(path: &Path, bytes: &[u8], name: &str, sequence: u64) -> Value {
    let raw_genesis = fs::read(path.join("genesis.json")).unwrap();
    let genesis: Value = serde_json::from_slice(&raw_genesis).unwrap();
    let original: Vec<u8> = serde_json::from_value(genesis["opening"].clone()).unwrap();
    assert_eq!(original, bytes, "literal opening byte-array must preserve all whitespace");
    assert_eq!(genesis["rules_identity"], RULES);
    let opening: Value = serde_json::from_slice(&original).unwrap();
    let expected: Value = serde_json::from_str(LEDGERS).unwrap();
    for row in expected[name]["accounts"].as_array().unwrap() {
        let account = opening["accounts"].as_array().unwrap().iter().find(|a| a["id"] == row[0]).unwrap();
        assert_eq!(account["cash"], row[2]);
        assert_eq!(account["source"], row[3]);
    }
    let current_raw = fs::read(path.join("CURRENT.json")).unwrap();
    let current: Value = serde_json::from_slice(&current_raw).unwrap();
    let slot = current["slot"].as_u64().unwrap();
    assert!(slot <= 1);
    let root_raw = fs::read(path.join(format!("root-{slot}.json"))).unwrap();
    let root: Value = serde_json::from_slice(&root_raw).unwrap();
    assert_eq!(current["root_sha256"].as_str(), Some(digest(&root_raw).as_str()));
    assert_eq!(root["genesis_sha256"].as_str(), Some(digest(&raw_genesis).as_str()));
    assert_ne!(root["genesis_sha256"].as_str(), Some(digest(bytes).as_str()));
    assert_eq!(root["sequence"].as_u64(), Some(sequence));
    json!({"opening_sha256":digest(bytes),"genesis_sha256":digest(&raw_genesis),"root":root,"current":current})
}

#[derive(Serialize, Deserialize)]
struct Probe { world: PathBuf, fixture: String, cash_delta: i64, sequence: u64 }
fn cold_open(artifacts: &Artifacts, world: &Path, name: &str, phase: &str, delta: i64, sequence: u64) {
    let probe = Probe { world: world.to_owned(), fixture: name.into(), cash_delta: delta, sequence };
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "cold_open_probe", "--ignored", "--nocapture", "--test-threads=1"])
        .env(PROBE_ENV, serde_json::to_string(&probe).unwrap())
        .output().expect("spawn same fixed test executable as a fresh recovery process");
    retain(&artifacts.case.join(format!("{phase}-stdout.log")), &output.stdout);
    retain(&artifacts.case.join(format!("{phase}-stderr.log")), &output.stderr);
    artifacts.record(&format!("{phase}-status"), &json!({"success":output.status.success(),"code":output.status.code(),"probe":probe}));
    assert!(output.status.success(), "cold recovery child failed; logs and world retained");
}

#[test]
#[ignore = "fresh-process helper; invoked explicitly by the six parent Store tests"]
fn cold_open_probe() {
    let root = configured_root();
    let probe: Probe = serde_json::from_str(&std::env::var(PROBE_ENV).expect("parent-provided probe required")).unwrap();
    let world = fs::canonicalize(&probe.world).unwrap();
    assert!(world.starts_with(&root) && world != root);
    let store = Store::open(&world, RULES).unwrap();
    let ledger = assert_ledger(&store, &probe.fixture, probe.cash_delta, probe.sequence);
    let hashes = assert_genesis(&world, &source_bytes(&probe.fixture), &probe.fixture, probe.sequence);
    println!("{}", json!({"fresh_process_pid":std::process::id(),"ledger":ledger,"genesis":hashes}));
}

#[test]
fn e0_same_prepared_slot_publishes_original_bytes_and_does_not_reissue_spent_capital() {
    let a = Artifacts::new("e0-same-slot");
    let world = a.world("world");
    let input = input("e0");
    let workspace = EngineWorkspace::new().unwrap();
    let before = workspace.identity().unwrap();
    a.record("prepared-identity", &serde_json::to_value(&before).unwrap());
    let mut points = Vec::new();
    let mut store = Store::create_opening_input_in_workspace_with_observer(
        &world, &input, resources("e0"), RULES, workspace,
        |point| { points.push(format!("{point:?}")); Ok(()) },
    ).unwrap();
    let after = store.model_workspace_identity().unwrap();
    assert_eq!(after.initial.world, before.initial.world);
    assert_eq!(after.initial.world_tables, before.initial.world_tables);
    assert_eq!(after.initial.engine_tables, before.initial.engine_tables);
    assert_eq!(after.initial.timeline_tables, before.initial.timeline_tables);
    assert_eq!(after.initial.background_tables, before.initial.background_tables);
    assert_eq!(after.initial.preparation_backing, before.initial.preparation_backing);
    assert_eq!(after.restore, before.restore);
    assert_eq!(after.candidate, before.candidate);
    assert_eq!(after.codec, before.codec);
    a.record("published-identity", &serde_json::to_value(&after).unwrap());
    a.record("create-points", &json!(points));
    assert_eq!(input.source().origin(), InputOrigin::Reader);
    assert_eq!(input.source().bytes(), source_bytes("e0"));
    assert_eq!(input.source().sha256(), digest(&source_bytes("e0")));
    a.record("initial-ledger", &assert_ledger(&store, "e0", 0, 0));
    a.record("initial-genesis", &assert_genesis(&world, input.source().bytes(), "e0", 0));
    let engine = store.engine().unwrap();
    let command = Envelope {
        world: 1, branch: 1, operation_id: 1, expected_revision: 0,
        actions: vec![Action::Transfer {
            from: engine.world().account_ref(1).unwrap(),
            to: engine.world().account_ref(4).unwrap(),
            amount: Money::new(137).unwrap(),
        }],
    };
    let outcome = store.execute(Caller::Account(1), command).unwrap();
    assert!(outcome.model.is_ok());
    a.record("spent-ledger", &assert_ledger(&store, "e0", 137, 1));
    drop(store);
    cold_open(&a, &world, "e0", "wal-fresh", 137, 1);
    let mut store = Store::open(&world, RULES).unwrap();
    assert_ledger(&store, "e0", 137, 1);
    store.checkpoint().unwrap();
    a.record("checkpoint-genesis", &assert_genesis(&world, input.source().bytes(), "e0", 1));
    drop(store);
    cold_open(&a, &world, "e0", "checkpoint-fresh", 137, 1);
    let prior = files(&world);
    assert!(Store::create_opening_input(&world, &input, resources("e0"), RULES).is_err());
    assert_eq!(files(&world), prior, "published directory retry must not overwrite any byte");
    cold_open(&a, &world, "e0", "published-retry-fresh", 137, 1);
}

fn finite_fixture_recovery(name: &str) {
    let a = Artifacts::new(name);
    let world = a.world("world");
    let input = input(name);
    let store = if name == "town" {
        Store::create_opening_input_spatial(&world, &input, resources(name), RULES, (SPATIAL, PNG))
    } else {
        Store::create_opening_input(&world, &input, resources(name), RULES)
    }.unwrap();
    a.record("initial-ledger", &assert_ledger(&store, name, 0, 0));
    a.record("initial-genesis", &assert_genesis(&world, input.source().bytes(), name, 0));
    drop(store);
    cold_open(&a, &world, name, "genesis-fresh", 0, 0);
    let mut store = Store::open(&world, RULES).unwrap();
    assert_ledger(&store, name, 0, 0);
    store.checkpoint().unwrap();
    drop(store);
    cold_open(&a, &world, name, "checkpoint-fresh", 0, 0);
}
#[test]
fn boot_original_capital_survives_checkpoint_and_fresh_process_reopen() { finite_fixture_recovery("boot"); }
#[test]
fn town_original_capital_survives_checkpoint_and_fresh_process_reopen() { finite_fixture_recovery("town"); }

#[test]
fn bad_schema_and_resources_cannot_publish_and_same_source_can_retry_in_new_directory() {
    let a = Artifacts::new("invalid-input");
    let boot = input("boot");
    let original = boot.source().bytes().to_vec();
    let hash = boot.source().sha256().to_owned();
    let failed = a.world("missing-resources");
    let result = Store::create_opening_input(&failed, &boot, vec![], RULES);
    assert!(matches!(&result, Err(Error::Model(message)) if message == "initial_resources"));
    a.record("resource-failure", &json!({"error":format!("{:?}",result.err()),"current":failed.join("CURRENT.json").exists()}));
    assert!(!failed.join("CURRENT.json").exists());
    assert_eq!(boot.source().bytes(), original);
    assert_eq!(boot.source().sha256(), hash);
    let retry = a.world("valid-resources-new-directory");
    let store = Store::create_opening_input(&retry, &boot, resources("boot"), RULES).unwrap();
    assert_ledger(&store, "boot", 0, 0);
    assert_genesis(&retry, &original, "boot", 0);
    drop(store);
    cold_open(&a, &retry, "boot", "valid-retry-fresh", 0, 0);
    let town = input("town");
    let no_bundle = a.world("town-missing-spatial-bundle");
    let result = Store::create_opening_input(&no_bundle, &town, resources("town"), RULES);
    assert!(matches!(&result, Err(Error::Invalid("exchange_spatial_bundle_required"))));
    assert!(!no_bundle.join("CURRENT.json").exists());
    a.record("town-missing-bundle", &json!({"error":format!("{:?}",result.err()),"source_sha256":town.source().sha256()}));
    assert_eq!(town.source().bytes(), source_bytes("town"));
    let bad = String::from_utf8(source_bytes("e0")).unwrap().replace("mycity-m0-opening-v1", "mycity-m0-opening-v99");
    let bad = OpeningInput::from_json(bad.as_bytes()).unwrap();
    for name in ["future-schema", "future-schema-new-directory"] {
        let path = a.world(name);
        let result = Store::create_opening_input(&path, &bad, vec![], RULES);
        assert!(matches!(&result, Err(Error::Model(message)) if message == "unsupported_opening_schema"));
        assert!(!path.join("CURRENT.json").exists());
        a.record(name, &json!({"error":format!("{:?}",result.err()),"source_sha256":bad.source().sha256()}));
    }
}

#[test]
fn create_io_cutpoints_require_reopen_after_current_and_keep_every_attempt() {
    let a = Artifacts::new("create-cutpoints");
    let input = input("e0");
    for (point, published) in [(CreatePoint::GenesisSynced,false),(CreatePoint::RootSynced,false),(CreatePoint::CurrentRenamed,true)] {
        let name = format!("{point:?}");
        let world = a.world(&name);
        let mut reached = false;
        let result = Store::create_opening_input_in_workspace_with_observer(
            &world, &input, vec![], RULES, EngineWorkspace::new().unwrap(),
            |at| { if at == point { reached = true; Err(io::Error::other("opening-input-create-injected")) } else { Ok(()) } },
        );
        assert!(reached, "observer point must actually be reached: {point:?}");
        assert!(matches!(&result, Err(Error::Io(error)) if error.to_string() == "opening-input-create-injected"));
        assert!(world.is_dir());
        assert_eq!(world.join("CURRENT.json").exists(), published);
        a.record(&format!("{name}-failure"), &json!({"point":name,"published_current":published,"error":format!("{:?}",result.err()),"files":files(&world)}));
        if published {
            // The callback returned an error after rename: outcome is uncertain, not a
            // create success response. Open the published world instead of blind create.
            cold_open(&a, &world, "e0", "current-renamed-recovery", 0, 0);
            let before = files(&world);
            assert!(Store::create_opening_input(&world, &input, vec![], RULES).is_err());
            assert_eq!(files(&world), before);
        } else {
            assert!(Store::open(&world, RULES).is_err());
            assert!(Store::create_opening_input(&world, &input, vec![], RULES).is_err());
            let retry = a.world(&format!("{name}-new-directory"));
            let store = Store::create_opening_input(&retry, &input, vec![], RULES).unwrap();
            assert_ledger(&store, "e0", 0, 0);
            drop(store);
            cold_open(&a, &retry, "e0", &format!("{name}-retry-fresh"), 0, 0);
        }
        assert_eq!(input.source().bytes(), source_bytes("e0"));
        assert_run_budget(&a.root);
    }
}

#[test]
fn legacy_raw_byte_store_create_keeps_original_genesis_and_recovery_contract() {
    let a = Artifacts::new("legacy-bytes");
    let world = a.world("world");
    let bytes = source_bytes("e0");
    let store = Store::create(&world, &bytes, vec![], RULES).unwrap();
    a.record("legacy-ledger", &assert_ledger(&store, "e0", 0, 0));
    a.record("legacy-genesis", &assert_genesis(&world, &bytes, "e0", 0));
    drop(store);
    cold_open(&a, &world, "e0", "legacy-fresh", 0, 0);
}
