//! Exact typed genesis bytes -> original Store slot -> original transactions/cold restore.
//! All evidence roots and uncertain publication directories are retained.
use mycity_core::{
    boot::{Command as Boot, Facility},
    opening_loader::OpeningInput,
    transaction::{Action, Caller, EngineWorkspace, Envelope, Resource},
    units::Money,
};
use mycity_storage::{CreatePoint, Store};
use serde_json::{json, Value};
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const BASE: &[u8] = include_bytes!("../../../tests/fixtures/opening-typed/base.json");
const BOOT: &[u8] = include_bytes!("../../../tests/fixtures/opening-typed/boot.json");
const RULES: &str = "typed-opening-137-v1";
static NEXT: AtomicU64 = AtomicU64::new(0);
fn case(name: &str) -> PathBuf {
    let root = PathBuf::from(
        std::env::var_os("MYCITY_TYPED_OPENING_STORE_ROOT")
            .expect("dedicated existing external SSD evidence root required"),
    )
    .canonicalize()
    .unwrap();
    assert!(root.starts_with("/Volumes/HP P900/dev/MyCity/work"));
    let p = root.join(format!(
        "{name}-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn record(p: &Path, name: &str, v: &Value) {
    fs::write(p.join(name), serde_json::to_vec_pretty(v).unwrap()).unwrap();
}
fn view(s: &Store) -> Value {
    serde_json::to_value(s.engine().unwrap().world().view()).unwrap()
}
fn ledger(s: &Store, spent: bool) {
    let v = view(s);
    assert_eq!(v["accounts"][0]["cash"], if spent { 4863 } else { 5000 });
    assert_eq!(v["accounts"][1]["cash"], if spent { 1137 } else { 1000 });
    assert_eq!(v["stocks"][0]["owner"], 2);
    assert_eq!(v["stocks"][0]["quantity"], 3000);
    assert_eq!(v["stocks"][0]["basis"], 700);
    assert_eq!(v["opening_sources"].as_array().unwrap().len(), 3);
}
fn transfer(s: &Store) -> Envelope {
    let e = s.engine().unwrap();
    Envelope {
        world: 9,
        branch: 3,
        operation_id: 77,
        expected_revision: e.world().view().revision,
        actions: vec![Action::Transfer {
            from: e.world().account_ref(1).unwrap(),
            to: e.world().account_ref(2).unwrap(),
            amount: Money::new(137).unwrap(),
        }],
    }
}
fn genesis(p: &Path, raw: &[u8]) {
    let v: Value = serde_json::from_slice(&fs::read(p.join("genesis.json")).unwrap()).unwrap();
    let retained: Vec<u8> = serde_json::from_value(v["opening"].clone()).unwrap();
    assert_eq!(retained, raw);
}
fn cold(p: &Path, world: &Path, mode: &str, label: &str) {
    let cfg = p.join(format!("cold-{label}.json"));
    record(
        p,
        cfg.file_name().unwrap().to_str().unwrap(),
        &json!({"world":world,"mode":mode}),
    );
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "cold_process_original_typed_genesis",
            "--nocapture",
        ])
        .env("MYCITY_TYPED_OPENING_PROBE", &cfg)
        .stdout(fs::File::create(p.join(format!("cold-{label}.stdout"))).unwrap())
        .stderr(fs::File::create(p.join(format!("cold-{label}.stderr"))).unwrap())
        .stdin(Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    let status = loop {
        if let Some(x) = child.try_wait().unwrap() {
            break x;
        }
        if start.elapsed() > Duration::from_secs(30) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("bounded own cold child timeout");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    record(
        p,
        &format!("cold-{label}-result.json"),
        &json!({"pid":child.id(),"exit":status.code(),"config":cfg}),
    );
    assert!(status.success());
}
#[test]
fn supplied_bytes_original_ticket_wal_checkpoint_and_fresh_process_never_reissue_capital() {
    let p = case("supplied");
    let world = p.join("world");
    let input = OpeningInput::from_json(BASE).unwrap();
    let mut s = Store::create_opening_input(&world, &input, vec![], RULES).unwrap();
    ledger(&s, false);
    genesis(&world, BASE);
    let req = transfer(&s);
    assert!(s
        .execute(Caller::Account(1), req.clone())
        .unwrap()
        .model
        .is_ok());
    ledger(&s, true);
    let hash = s.state_hash().unwrap();
    let seq = s.committed_sequence();
    drop(s);
    cold(&p, &world, "spent", "wal");
    let mut s = Store::open(&world, RULES).unwrap();
    assert!(s
        .execute(Caller::Account(1), req.clone())
        .unwrap()
        .model
        .is_ok());
    assert_eq!(s.state_hash().unwrap(), hash);
    assert_eq!(s.committed_sequence(), seq);
    s.checkpoint().unwrap();
    drop(s);
    cold(&p, &world, "spent", "checkpoint");
    assert!(Store::create_opening_input(&world, &input, vec![], RULES).is_err());
    let s = Store::open(&world, RULES).unwrap();
    ledger(&s, true);
    record(
        &p,
        "original-operation.json",
        &serde_json::to_value(req).unwrap(),
    );
    record(
        &p,
        "result.json",
        &json!({"hash":hash,"sequence":seq,"actual":view(&s)}),
    );
}
#[test]
fn selected_reader_original_bytes_survive_external_source_change_without_reinitialization() {
    let p = case("reader");
    let file = p.join("selected.json");
    fs::write(&file, BASE).unwrap();
    let input =
        OpeningInput::read_named_json(fs::File::open(&file).unwrap(), file.to_str().unwrap())
            .unwrap();
    let world = p.join("world");
    let s = Store::create_opening_input(&world, &input, vec![], RULES).unwrap();
    ledger(&s, false);
    genesis(&world, BASE);
    drop(s);
    let mut changed: Value = serde_json::from_slice(BASE).unwrap();
    changed["accounts"][0]["cash"]["value"] = json!(9000);
    changed["sources"][0]["amount"]["value"] = json!(9000);
    fs::write(&file, serde_json::to_vec(&changed).unwrap()).unwrap();
    cold(&p, &world, "initial", "external-changed");
    let s = Store::open(&world, RULES).unwrap();
    ledger(&s, false);
    genesis(&world, BASE);
    record(&p, "result.json", &view(&s));
}
#[test]
fn typed_boot_actual_start_split_clock_and_completion_survive_original_cold_store() {
    let p = case("boot");
    let world = p.join("world");
    let input = OpeningInput::from_json(BOOT).unwrap();
    let resources = vec![
        Resource::new(102, 2).unwrap(),
        Resource::new(101, 2).unwrap(),
    ];
    let mut s = Store::create_opening_input(&world, &input, resources, RULES).unwrap();
    let mut requests = Vec::new();
    for command in [
        Boot::Start {
            facility: Facility::Pump,
            reverse_request: false,
        },
        Boot::Advance { seconds: 30 },
    ] {
        let e = s.engine().unwrap();
        let request = Envelope {
            world: 1,
            branch: 1,
            operation_id: s.committed_sequence() + 1,
            expected_revision: e.world().view().revision,
            actions: vec![Action::Boot { command }],
        };
        assert!(s
            .execute(Caller::System, request.clone())
            .unwrap()
            .model
            .is_ok());
        requests.push(request);
    }
    assert_eq!(view(&s)["boot"]["time"], 30);
    s.checkpoint().unwrap();
    drop(s);
    cold(&p, &world, "boot30", "midway");
    let mut s = Store::open(&world, RULES).unwrap();
    let before = s.state_hash().unwrap();
    for req in requests {
        assert!(s.execute(Caller::System, req).unwrap().model.is_ok());
    }
    assert_eq!(s.state_hash().unwrap(), before);
    let req = Envelope {
        world: 1,
        branch: 1,
        operation_id: s.committed_sequence() + 1,
        expected_revision: s.engine().unwrap().world().view().revision,
        actions: vec![Action::Boot {
            command: Boot::Advance { seconds: 30 },
        }],
    };
    assert!(s.execute(Caller::System, req).unwrap().model.is_ok());
    let v = view(&s);
    assert_eq!(v["boot"]["time"], 60);
    assert_eq!(
        v["stocks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == 3)
            .unwrap()["quantity"],
        600000
    );
    s.checkpoint().unwrap();
    drop(s);
    cold(&p, &world, "boot60", "completed");
    genesis(&world, BOOT);
}
#[test]
fn semantic_or_resource_errors_leave_no_published_current_and_input_is_retained() {
    let p = case("reject");
    for (index, (pointer, value)) in [
        ("/stocks/0/owner/domain", json!("resource")),
        ("/stocks/0/quantity/unit", json!("millilitre")),
        ("/provider/version", json!(2)),
        ("/accounts/1/source/id", json!(10)),
    ]
    .into_iter()
    .enumerate()
    {
        let mut v: Value = serde_json::from_slice(BASE).unwrap();
        *v.pointer_mut(pointer).unwrap() = value;
        let bytes = serde_json::to_vec(&v).unwrap();
        let input = OpeningInput::from_json(&bytes).unwrap();
        let world = p.join(format!("invalid-{index}"));
        let result = Store::create_opening_input(&world, &input, vec![], RULES);
        assert!(result.is_err());
        assert!(!world.join("CURRENT.json").exists());
        assert_eq!(input.source().bytes(), bytes);
        record(
            &p,
            &format!("invalid-{index}.json"),
            &json!({"input":v,"error":format!("{:?}",result.err())}),
        );
    }
    let world = p.join("wrong-resource");
    assert!(Store::create_opening_input(
        &world,
        &OpeningInput::from_json(BOOT).unwrap(),
        vec![],
        RULES
    )
    .is_err());
    assert!(!world.join("CURRENT.json").exists());
}
#[test]
fn publication_interruption_preserves_same_slot_and_lost_response_is_resolved_by_open() {
    let p = case("publication");
    let input = OpeningInput::from_json(BASE).unwrap();
    for (point, published) in [
        (CreatePoint::GenesisSynced, false),
        (CreatePoint::RootSynced, false),
        (CreatePoint::CurrentRenamed, true),
    ] {
        let world = p.join(format!("{point:?}"));
        let result = Store::create_opening_input_in_workspace_with_observer(
            &world,
            &input,
            vec![],
            RULES,
            EngineWorkspace::new().unwrap(),
            |at| {
                if at == point {
                    Err(io::Error::other(
                        "normal interrupted initialization; originals retained",
                    ))
                } else {
                    Ok(())
                }
            },
        );
        assert!(result.is_err());
        record(
            &p,
            &format!("{point:?}-failure.json"),
            &json!({"point":format!("{point:?}"),"error":format!("{:?}",result.err()),"published":published}),
        );
        assert_eq!(world.join("CURRENT.json").exists(), published);
        if published {
            cold(&p, &world, "initial", "lost-response");
            let s = Store::open(&world, RULES).unwrap();
            ledger(&s, false);
            drop(s);
            assert!(Store::create_opening_input(&world, &input, vec![], RULES).is_err());
        } else {
            assert!(Store::open(&world, RULES).is_err());
        }
    }
}
#[test]
#[ignore = "invoked only by real parent fresh-process scenarios"]
fn cold_process_original_typed_genesis() {
    let cfg = PathBuf::from(
        std::env::var_os("MYCITY_TYPED_OPENING_PROBE").expect("only parent may invoke"),
    );
    let input: Value = serde_json::from_slice(&fs::read(cfg).unwrap()).unwrap();
    let s = Store::open(Path::new(input["world"].as_str().unwrap()), RULES).unwrap();
    match input["mode"].as_str().unwrap() {
        "initial" => ledger(&s, false),
        "spent" => ledger(&s, true),
        "boot30" | "boot60" => {
            let expected = if input["mode"] == "boot30" { 30 } else { 60 };
            assert_eq!(view(&s)["boot"]["time"], expected);
            let expected_water = if expected == 30 { 0 } else { 600000 };
            assert_eq!(
                view(&s)["stocks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|v| v["id"] == 3)
                    .unwrap()["quantity"],
                expected_water
            );
        }
        _ => panic!("unknown parent oracle"),
    };
    println!("original typed cold state {}", s.state_hash().unwrap());
}
