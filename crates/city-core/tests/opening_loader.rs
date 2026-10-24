// Production transport source and historical diagnostics remain separate.
pub use mycity_core::{assembly, read_bounded, sha256, transaction, INPUT_LIMIT};
#[allow(dead_code)]
#[path = "fixtures/issue-120/diagnostic-v3/opening_loader.rs"]
mod diagnostic_v3_loader;
#[path = "../src/opening_loader.rs"]
mod opening_loader;
use opening_loader::{InputOrigin, OpeningInput, MAX_SOURCE_LABEL_BYTES};
// Old seven diagnostics intentionally retain their failed API and direct assembler.
use diagnostic_v3_loader::LoadedOpening;
use mycity_core::{
    canonical::Codec,
    transaction::{Engine, EngineSlot, Resource},
};
use serde_json::{json, Value};
use std::cell::Cell;
use std::io::{self, Read};
use std::rc::Rc;

const SIMPLE: &[u8] = include_bytes!("fixtures/issue-120/canonical/m0-e0-opening.json");
const BOOT: &[u8] = include_bytes!("fixtures/issue-120/canonical/m0-boot-opening.json");
const EXISTING: &[(&str, &[u8])] = &[
    ("e0", SIMPLE),
    (
        "region",
        include_bytes!("fixtures/issue-120/canonical/m0-region-opening.json"),
    ),
    (
        "economy",
        include_bytes!("fixtures/issue-120/canonical/m0-economy-opening.json"),
    ),
    ("boot", BOOT),
    (
        "exchange",
        include_bytes!("fixtures/issue-120/canonical/m0-exchange-opening.json"),
    ),
    (
        "background-joint",
        include_bytes!("fixtures/issue-120/canonical/m0-background-joint-opening.json"),
    ),
    (
        "town-utility",
        include_bytes!("fixtures/issue-120/canonical/core-town-utility-opening.json"),
    ),
];

// Test caller resources copied from existing canonical boot/exchange/town tests.
// These laboratory values never become defaults in the production adapter.
fn resources_for(name: &str) -> Vec<Resource> {
    let rows: &[(u32, u32)] = match name {
        "boot" => &[(102, 2), (101, 2)],
        "exchange" | "background-joint" | "town-utility" => &[
            (101, 1),
            (102, 1),
            (103, 1),
            (104, 1),
            (201, 2000),
            (202, 2000),
        ],
        _ => &[],
    };
    rows.iter()
        .map(|&(id, cap)| Resource::new(id, cap).unwrap())
        .collect()
}

// Borrowed canonical serialization avoids constructing/moving a large WorldView.
// This codec and output are test observation costs, not free product workspace.
fn engine_snapshot(engine: &Engine) -> (String, Vec<u8>) {
    eprintln!("issue120:retained-observer:before-codec-new");
    let mut codec = Codec::new().unwrap();
    eprintln!("issue120:retained-observer:codec-new-returned-before-write");
    let mut bytes = Vec::new();
    engine
        .write_state_comparison(&mut codec, &mut bytes)
        .unwrap();
    eprintln!(
        "issue120:retained-observer:write-returned-before-hash output_bytes={}",
        bytes.len()
    );
    let hash = engine.state_hash_with(&mut codec).unwrap();
    eprintln!("issue120:retained-observer:hash-returned");
    (hash, bytes)
}

#[inline(never)]
fn retained_snapshot(bytes: &[u8], resources: &[Resource]) -> (String, Vec<u8>) {
    eprintln!(
        "issue120:retained-direct:before-slot-new input_bytes={}",
        bytes.len()
    );
    let mut slot = EngineSlot::new().unwrap();
    eprintln!("issue120:retained-direct:slot-new-returned-before-initialize");
    slot.initialize_into(bytes, resources).unwrap();
    eprintln!("issue120:retained-direct:initialize-returned-before-observer");
    let observed = engine_snapshot(slot.engine().unwrap());
    eprintln!("issue120:retained-direct:observer-returned-before-drop");
    drop(slot);
    eprintln!("issue120:retained-direct:drop-returned");
    observed
}

#[inline(never)]
fn input_snapshot(bytes: &[u8], resources: &[Resource], streamed: bool) -> (String, Vec<u8>) {
    eprintln!(
        "issue120:retained-input:before-transport streamed={streamed} input_bytes={}",
        bytes.len()
    );
    let input = if streamed {
        OpeningInput::read_json(bytes).unwrap()
    } else {
        OpeningInput::from_json(bytes).unwrap()
    };
    assert_eq!(input.source().bytes(), bytes);
    assert_eq!(input.source().sha256(), sha256(bytes));
    assert_eq!(
        input.source().origin(),
        if streamed {
            InputOrigin::Reader
        } else {
            InputOrigin::SuppliedBytes
        }
    );
    assert_eq!(input.source().label(), None);
    eprintln!("issue120:retained-input:transport-returned-before-slot-new");
    let mut slot = EngineSlot::new().unwrap();
    eprintln!("issue120:retained-input:slot-new-returned-before-initialize");
    let observed = {
        let initialized = input.initialize_into(&mut slot, resources).unwrap();
        eprintln!("issue120:retained-input:initialize-returned-before-observer");
        assert_eq!(initialized.source().bytes(), bytes);
        engine_snapshot(initialized.engine())
    };
    eprintln!("issue120:retained-input:observer-returned-before-drop");
    drop(slot);
    eprintln!("issue120:retained-input:drop-returned");
    observed
}

fn rejection(bytes: &[u8], resources: &[Resource]) -> String {
    let expected = {
        let mut slot = EngineSlot::new().unwrap();
        let error = slot.initialize_into(bytes, resources).unwrap_err();
        assert!(slot.engine().is_err());
        error
    };
    for streamed in [false, true] {
        let input = if streamed {
            OpeningInput::read_json(bytes)
        } else {
            OpeningInput::from_json(bytes)
        };
        let actual = match input {
            Err(error) => error,
            Ok(input) => {
                let mut slot = EngineSlot::new().unwrap();
                let error = match input.initialize_into(&mut slot, resources) {
                    Err(error) => error,
                    Ok(_) => panic!("adapter accepted bytes rejected by retained initializer"),
                };
                assert!(slot.engine().is_err());
                error
            }
        };
        assert_eq!(actual, expected);
    }
    expected
}

fn mutated(change: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(SIMPLE).unwrap();
    change(&mut value);
    serde_json::to_vec(&value).unwrap()
}

// Exactly the same complete three-path comparison is used by the six-oracle suite
// and each fresh-process fixture diagnostic; no failing fixture/assertion is removed.
fn compare_existing_fixture(name: &str, bytes: &[u8]) {
    let resources = resources_for(name);
    eprintln!("issue120:fixture:{name}:direct-retained:start");
    let expected = retained_snapshot(bytes, &resources);
    eprintln!("issue120:fixture:{name}:direct-retained:complete;bytes:start");
    assert_eq!(
        input_snapshot(bytes, &resources, false),
        expected,
        "{name}: bytes"
    );
    eprintln!("issue120:fixture:{name}:bytes:complete;reader:start");
    assert_eq!(
        input_snapshot(bytes, &resources, true),
        expected,
        "{name}: reader"
    );
    eprintln!("issue120:fixture:{name}:reader:complete");
}

#[test]
fn existing_openings_have_identical_worlds_and_exact_source_bytes() {
    for (name, bytes) in EXISTING {
        compare_existing_fixture(name, bytes);
    }
}

#[test]
fn rejects_bad_schema_unknown_fields_units_and_entitlements_with_original_errors() {
    let changes: &[fn(&mut Value)] = &[
        |v| v["schema"] = json!("mycity-m0-opening-v99"),
        |v| v["bootstrap"]["schema"] = json!("mycity-m0-bootstrap-v99"),
        |v| v["bootstrap"]["laboratory_only"] = json!(false),
        |v| v["bootstrap"]["world"] = json!(0),
        |v| v["unrecognized"] = json!(true),
        |v| v["accounts"][0]["unit"] = json!("dollar"),
        |v| v["accounts"][0]["cash"] = json!("100 dollars"),
        |v| v["accounts"][0]["cash"] = json!(-1),
        |v| v["accounts"][0]["source"] = json!(999),
        |v| v["accounts"][1]["source"] = json!(1),
        |v| v["accounts"][1]["id"] = json!(1),
        |v| v["sources"][0]["amount"] = json!(999),
        |v| v["sources"][0]["kind"] = json!("inventory_capital"),
        |v| v["stocks"][0]["owner"] = json!(999),
        |v| v["stocks"][0]["quantity"] = json!(0),
        |v| v["stocks"][0]["quantity"] = json!(-1),
        |v| {
            v["sources"]
                .as_array_mut()
                .unwrap()
                .push(json!({"id":99,"kind":"cash_capital","amount":1}))
        },
        |v| {
            v.as_object_mut().unwrap().remove("accounts");
        },
        |v| v["accounts"] = json!([]),
    ];
    for change in changes {
        rejection(&mutated(*change), &[]);
    }
    let mut nested: Value = serde_json::from_slice(BOOT).unwrap();
    nested["boot"]["profile"] = json!("unrecognized-profile-v99");
    rejection(
        &serde_json::to_vec(&nested).unwrap(),
        &resources_for("boot"),
    );
    nested["boot"]["unknown"] = json!(1);
    rejection(
        &serde_json::to_vec(&nested).unwrap(),
        &resources_for("boot"),
    );
}

#[test]
fn rejects_count_overflow_bad_syntax_and_bad_utf8() {
    let rows = mutated(|v| {
        let row = v["accounts"][0].clone();
        v["accounts"] = json!(vec![row; assembly::MAX_ACCOUNTS + 1]);
    });
    assert_eq!(rejection(&rows, &[]), "opening_count_limit");
    rejection(b"{", &[]);
    rejection(&[0xff], &[]);
    let mut trailing = SIMPLE.to_vec();
    trailing.extend_from_slice(b"{}");
    rejection(&trailing, &[]);
}

#[test]
fn accepts_exact_byte_limit_and_rejects_one_more_byte() {
    let mut exact = SIMPLE.to_vec();
    exact.resize(INPUT_LIMIT as usize, b' ');
    let expected = retained_snapshot(&exact, &[]);
    assert_eq!(input_snapshot(&exact, &[], false), expected);
    assert_eq!(input_snapshot(&exact, &[], true), expected);
    exact.push(b' ');
    assert_eq!(rejection(&exact, &[]), "input_too_large");
}

#[test]
fn source_identity_survives_handoff_without_json_normalization() {
    let expected = retained_snapshot(SIMPLE, &[]);
    let mut bytes = b" \n".to_vec();
    bytes.extend_from_slice(SIMPLE);
    bytes.extend_from_slice(b"\r\n ");
    let label = "fixture://canonical/e0-laboratory";
    let input = OpeningInput::read_named_json(bytes.as_slice(), label).unwrap();
    let observed = {
        let mut slot = EngineSlot::new().unwrap();
        let initialized = input.initialize_into(&mut slot, &[]).unwrap();
        assert_eq!(initialized.source().label(), Some(label));
        assert_eq!(initialized.source().bytes(), bytes);
        engine_snapshot(initialized.engine())
    };
    assert_eq!(observed, expected);
    let source = input.into_source();
    assert_eq!(source.bytes(), bytes);
    assert_eq!(source.sha256(), sha256(&bytes));
    assert_ne!(source.sha256(), sha256(SIMPLE));
    assert_eq!(source.origin(), InputOrigin::Reader);
    assert_eq!(source.label(), Some(label));
    // This is an ownership/source-transfer oracle, not actual Store disk recovery.
}

struct EndlessReader(Rc<Cell<usize>>);
impl Read for EndlessReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        output.fill(b' ');
        self.0.set(self.0.get() + output.len());
        Ok(output.len())
    }
}
struct BrokenReader;
impl Read for BrokenReader {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "provider_read_denied",
        ))
    }
}

#[test]
fn bounds_untrusted_stream_and_preserves_real_read_errors() {
    let count = Rc::new(Cell::new(0));
    assert!(
        matches!(OpeningInput::read_json(EndlessReader(count.clone())), Err(e) if e == "input_too_large")
    );
    assert_eq!(count.get(), INPUT_LIMIT as usize + 1);
    assert!(matches!(OpeningInput::read_json(BrokenReader), Err(e) if e == "provider_read_denied"));
    let label = "x".repeat(MAX_SOURCE_LABEL_BYTES);
    assert_eq!(
        OpeningInput::read_named_json(SIMPLE, &label)
            .unwrap()
            .source()
            .label(),
        Some(label.as_str())
    );
    let count = Rc::new(Cell::new(0));
    assert!(
        matches!(OpeningInput::read_named_json(EndlessReader(count.clone()), &(label + "x")), Err(e) if e == "opening_loader_source_label_too_large")
    );
    assert_eq!(count.get(), 0);

    // Loading is unvalidated transport. Failed initialization leaves the caller's slot
    // unpublished and source intact; retry in the same invalid slot starts from reset.
    let bad = OpeningInput::from_json(b"{").unwrap();
    let good = OpeningInput::from_json(SIMPLE).unwrap();
    let mut slot = EngineSlot::new().unwrap();
    assert!(bad.initialize_into(&mut slot, &[]).is_err());
    assert!(slot.engine().is_err());
    assert_eq!(bad.source().bytes(), b"{");
    let incomplete = OpeningInput::from_json(BOOT).unwrap();
    assert!(
        matches!(incomplete.initialize_into(&mut slot, &[]), Err(e) if e == "initial_resources")
    );
    assert!(slot.engine().is_err());
    // Caller-supplied resource failures use the same retained authority and leave no root.
    let mut reserved = Resource::new(1, 1).unwrap();
    reserved.reserved = 1;
    rejection(SIMPLE, &[reserved]);
    let duplicate = Resource::new(1, 1).unwrap();
    rejection(SIMPLE, &[duplicate, duplicate]);
    let too_many = vec![duplicate; transaction::MAX_RESOURCES + 1];
    assert_eq!(rejection(SIMPLE, &too_many), "initial_resource_limit");
    let first = {
        let initialized = good.initialize_into(&mut slot, &[]).unwrap();
        engine_snapshot(initialized.engine())
    };
    assert!(
        matches!(good.initialize_into(&mut slot, &[]), Err(e) if e == "opening_loader_slot_already_initialized")
    );
    assert_eq!(engine_snapshot(slot.engine().unwrap()), first);
}

// Historical default-stack diagnostic path, intentionally excluded from acceptance.
#[inline(never)]
fn direct_snapshot(bytes: &[u8]) -> (String, Vec<u8>) {
    eprintln!("issue120:direct-snapshot:before-assemble");
    let world = assembly::assemble(bytes).unwrap();
    eprintln!("issue120:direct-snapshot:assemble-returned-before-hash");
    let hash = world.state_hash();
    eprintln!("issue120:direct-snapshot:hash-returned-before-view");
    let view = serde_json::to_vec(&world.view()).unwrap();
    eprintln!("issue120:direct-snapshot:view-returned");
    (hash, view)
}

// Each ignored diagnostic must be selected with --exact --ignored --nocapture in
// its own process. They localize a fatal abort; they do not replace the six oracles.
// No global/thread stack adjustment or extra modeled budget is introduced.
fn diagnostic_bytes(padded: bool) -> Vec<u8> {
    let mut bytes = SIMPLE.to_vec();
    if padded {
        bytes.resize(INPUT_LIMIT as usize, b' ');
    }
    bytes
}

#[inline(never)]
fn diagnostic_direct_only(bytes: &[u8]) {
    eprintln!(
        "issue120:direct-only:entered-before-assemble bytes={}",
        bytes.len()
    );
    let world = assembly::assemble(bytes).unwrap();
    eprintln!("issue120:direct-only:assemble-returned-before-drop");
    drop(world);
    eprintln!("issue120:direct-only:drop-returned");
}

#[test]
#[ignore = "isolated fatal-stack diagnostic; run exact with nocapture"]
fn diagnostic_00_type_sizes() {
    eprintln!(
        "issue120:type-sizes World={} Opening={} WorldView={} LoadedOpening={}",
        std::mem::size_of::<assembly::World>(),
        std::mem::size_of::<assembly::Opening>(),
        std::mem::size_of::<assembly::WorldView>(),
        std::mem::size_of::<LoadedOpening>(),
    );
    // These are inline type sizes, not physical stack frames or heap peak evidence.
}

#[test]
#[ignore = "isolated fatal-stack diagnostic; run exact with nocapture"]
fn diagnostic_01_direct_simple() {
    eprintln!("issue120:diagnostic-01:before-helper");
    diagnostic_direct_only(SIMPLE);
    eprintln!("issue120:diagnostic-01:complete");
}

#[test]
#[ignore = "isolated fatal-stack diagnostic; run exact with nocapture"]
fn diagnostic_02_direct_padded() {
    let bytes = diagnostic_bytes(true);
    eprintln!("issue120:diagnostic-02:before-helper");
    diagnostic_direct_only(&bytes);
    eprintln!("issue120:diagnostic-02:complete");
}

#[test]
#[ignore = "isolated fatal-stack diagnostic; run exact with nocapture"]
fn diagnostic_03_direct_snapshot_padded() {
    let bytes = diagnostic_bytes(true);
    eprintln!("issue120:diagnostic-03:before-helper");
    let observed = direct_snapshot(&bytes);
    eprintln!(
        "issue120:diagnostic-03:complete hash={} view_bytes={}",
        observed.0,
        observed.1.len()
    );
}

#[test]
#[ignore = "isolated fatal-stack diagnostic; run exact with nocapture"]
fn diagnostic_04_loader_bytes_padded() {
    let bytes = diagnostic_bytes(true);
    eprintln!("issue120:diagnostic-04:before-from-json");
    let loaded = LoadedOpening::from_json(&bytes).unwrap();
    eprintln!(
        "issue120:diagnostic-04:loaded-before-drop source_bytes={}",
        loaded.source().bytes().len()
    );
    drop(loaded);
    eprintln!("issue120:diagnostic-04:complete");
}

#[test]
#[ignore = "isolated fatal-stack diagnostic; run exact with nocapture"]
fn diagnostic_05_loader_reader_padded() {
    let bytes = diagnostic_bytes(true);
    eprintln!("issue120:diagnostic-05:before-read-json");
    let loaded = LoadedOpening::read_json(bytes.as_slice()).unwrap();
    eprintln!(
        "issue120:diagnostic-05:loaded-before-drop source_bytes={}",
        loaded.source().bytes().len()
    );
    drop(loaded);
    eprintln!("issue120:diagnostic-05:complete");
}

#[test]
#[ignore = "isolated existing retained-slot diagnostic; not Store acceptance"]
fn diagnostic_06_retained_slot_padded() {
    let bytes = diagnostic_bytes(true);
    eprintln!("issue120:diagnostic-06:before-existing-slot-new");
    let mut slot = mycity_core::transaction::EngineSlot::new().unwrap();
    eprintln!("issue120:diagnostic-06:slot-new-returned-before-initialize");
    slot.initialize_into(&bytes, &[]).unwrap();
    eprintln!("issue120:diagnostic-06:initialize-returned-before-drop");
    drop(slot);
    eprintln!("issue120:diagnostic-06:complete");
}

#[test]
#[ignore = "fresh-process retained fixture localization; not replacement acceptance"]
fn diagnostic_retained_fixture_00_e0() {
    let (name, bytes) = EXISTING[0];
    compare_existing_fixture(name, bytes);
}

#[test]
#[ignore = "fresh-process retained fixture localization; not replacement acceptance"]
fn diagnostic_retained_fixture_01_region() {
    let (name, bytes) = EXISTING[1];
    compare_existing_fixture(name, bytes);
}

#[test]
#[ignore = "fresh-process retained fixture localization; not replacement acceptance"]
fn diagnostic_retained_fixture_02_economy() {
    let (name, bytes) = EXISTING[2];
    compare_existing_fixture(name, bytes);
}

#[test]
#[ignore = "fresh-process retained fixture localization; not replacement acceptance"]
fn diagnostic_retained_fixture_03_boot() {
    let (name, bytes) = EXISTING[3];
    compare_existing_fixture(name, bytes);
}

#[test]
#[ignore = "fresh-process retained fixture localization; not replacement acceptance"]
fn diagnostic_retained_fixture_04_exchange() {
    let (name, bytes) = EXISTING[4];
    compare_existing_fixture(name, bytes);
}

#[test]
#[ignore = "fresh-process retained fixture localization; not replacement acceptance"]
fn diagnostic_retained_fixture_05_background_joint() {
    let (name, bytes) = EXISTING[5];
    compare_existing_fixture(name, bytes);
}

#[test]
#[ignore = "fresh-process retained fixture localization; not replacement acceptance"]
fn diagnostic_retained_fixture_06_town_utility() {
    let (name, bytes) = EXISTING[6];
    compare_existing_fixture(name, bytes);
}
