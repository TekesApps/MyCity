// Include the exclusive source without editing the shared crate root. Integration must
// compile this against the coordinator's exact canonical API snapshot, including WIP.
pub use mycity_core::{assembly, read_bounded, sha256, INPUT_LIMIT};

#[path = "../src/opening_loader.rs"]
mod opening_loader;

use opening_loader::{InputOrigin, LoadedOpening, MAX_SOURCE_LABEL_BYTES};
use serde_json::{json, Value};
use std::cell::Cell;
use std::io::{self, Read};
use std::rc::Rc;

const SIMPLE: &[u8] = include_bytes!("fixtures/issue-120/canonical/m0-e0-opening.json");
const BOOT: &[u8] = include_bytes!("fixtures/issue-120/canonical/m0-boot-opening.json");
const EXISTING: &[(&str, &[u8])] = &[
    ("e0", SIMPLE),
    ("region", include_bytes!("fixtures/issue-120/canonical/m0-region-opening.json")),
    ("economy", include_bytes!("fixtures/issue-120/canonical/m0-economy-opening.json")),
    ("boot", BOOT),
    ("exchange", include_bytes!("fixtures/issue-120/canonical/m0-exchange-opening.json")),
    ("background-joint", include_bytes!("fixtures/issue-120/canonical/m0-background-joint-opening.json")),
    ("town-utility", include_bytes!("fixtures/issue-120/canonical/core-town-utility-opening.json")),
];

// Keep the single direct World return temporary in its own frame. No World escapes.
#[inline(never)]
fn canonical_rejection(bytes: &[u8]) -> String {
    match assembly::assemble(bytes) {
        Err(error) => error,
        Ok(_) => panic!("negative fixture unexpectedly accepted by canonical assembler"),
    }
}

fn rejection(bytes: &[u8]) -> String {
    let expected = canonical_rejection(bytes);
    let actual = match LoadedOpening::from_json(bytes) {
        Err(error) => error,
        Ok(_) => panic!("loader accepted input rejected by canonical assembler"),
    };
    assert_eq!(actual, expected);
    let streamed = match LoadedOpening::read_json(bytes) {
        Err(error) => error,
        Ok(_) => panic!("stream loader accepted rejected bytes"),
    };
    assert_eq!(streamed, expected);
    expected
}

fn mutated(change: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut value: Value = serde_json::from_slice(SIMPLE).unwrap();
    change(&mut value);
    serde_json::to_vec(&value).unwrap()
}

// Snapshot helpers return heap-owned observations, never World or WorldView. Completing
// each call drops its only World before the next path assembles another one. These frame
// boundaries also prevent debug builds from reserving several inline World slots together.
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

#[inline(never)]
fn loaded_snapshot(bytes: &[u8], streamed: bool) -> (String, Vec<u8>) {
    let loaded = if streamed {
        LoadedOpening::read_json(bytes).unwrap()
    } else {
        LoadedOpening::from_json(bytes).unwrap()
    };
    assert_eq!(loaded.source().bytes(), bytes);
    assert_eq!(loaded.source().sha256(), sha256(bytes));
    assert_eq!(
        loaded.source().origin(),
        if streamed { InputOrigin::Reader } else { InputOrigin::SuppliedBytes },
    );
    assert_eq!(loaded.source().label(), None);
    (loaded.world().state_hash(), serde_json::to_vec(&loaded.world().view()).unwrap())
}

#[test]
fn existing_openings_have_identical_worlds_and_exact_source_bytes() {
    for (name, bytes) in EXISTING {
        let direct = direct_snapshot(bytes);
        assert_eq!(loaded_snapshot(bytes, false), direct, "{name}: supplied bytes");
        assert_eq!(loaded_snapshot(bytes, true), direct, "{name}: reader");
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
        |v| v["sources"].as_array_mut().unwrap().push(json!({"id":99,"kind":"cash_capital","amount":1})),
        |v| { v.as_object_mut().unwrap().remove("accounts"); },
        |v| v["accounts"] = json!([]),
    ];
    for change in changes {
        rejection(&mutated(*change));
    }
    let mut nested: Value = serde_json::from_slice(BOOT).unwrap();
    nested["boot"]["profile"] = json!("unrecognized-profile-v99");
    rejection(&serde_json::to_vec(&nested).unwrap());
    nested["boot"]["unknown"] = json!(1);
    rejection(&serde_json::to_vec(&nested).unwrap());
}

#[test]
fn rejects_count_overflow_bad_syntax_and_bad_utf8() {
    let rows = mutated(|v| {
        let row = v["accounts"][0].clone();
        v["accounts"] = json!(vec![row; assembly::MAX_ACCOUNTS + 1]);
    });
    assert_eq!(rejection(&rows), "opening_count_limit");
    rejection(b"{");
    rejection(&[0xff]);
    let mut trailing = SIMPLE.to_vec();
    trailing.extend_from_slice(b"{}");
    rejection(&trailing);
}

#[test]
fn accepts_exact_byte_limit_and_rejects_one_more_byte() {
    let mut exact = SIMPLE.to_vec();
    exact.resize(INPUT_LIMIT as usize, b' ');
    let expected = direct_snapshot(&exact);
    assert_eq!(loaded_snapshot(&exact, false), expected);
    assert_eq!(loaded_snapshot(&exact, true), expected);
    exact.push(b' ');
    assert_eq!(rejection(&exact), "input_too_large");
}

#[inline(never)]
fn handoff_snapshot(bytes: &[u8], label: &str) -> (String, opening_loader::OpeningSource) {
    let loaded = LoadedOpening::read_named_json(bytes, label).unwrap();
    let (world, source) = loaded.into_parts();
    // The boxed World is dropped here before the direct path starts in the test body.
    (world.state_hash(), source)
}

#[test]
fn source_identity_survives_handoff_without_json_normalization() {
    let mut bytes = b" \n".to_vec();
    bytes.extend_from_slice(SIMPLE);
    bytes.extend_from_slice(b"\r\n ");
    let label = "fixture://canonical/e0-laboratory";
    let (state_hash, source) = handoff_snapshot(&bytes, label);
    assert_eq!(source.bytes(), bytes);
    assert_eq!(source.sha256(), sha256(&bytes));
    assert_ne!(source.sha256(), sha256(SIMPLE));
    assert_eq!(source.origin(), InputOrigin::Reader);
    assert_eq!(source.label(), Some(label));
    assert_eq!(state_hash, direct_snapshot(SIMPLE).0);
    // This checks source transfer only. Actual Store create/open/checkpoint recovery
    // must be exercised by the coordinator at the existing Store boundary.
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
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "provider_read_denied"))
    }
}

#[test]
fn bounds_untrusted_stream_and_preserves_real_read_errors() {
    let count = Rc::new(Cell::new(0));
    assert!(matches!(
        LoadedOpening::read_json(EndlessReader(count.clone())),
        Err(error) if error == "input_too_large"
    ));
    assert_eq!(count.get(), INPUT_LIMIT as usize + 1);
    assert!(matches!(
        LoadedOpening::read_json(BrokenReader),
        Err(error) if error == "provider_read_denied"
    ));
    let label = "x".repeat(MAX_SOURCE_LABEL_BYTES);
    assert_eq!(
        LoadedOpening::read_named_json(SIMPLE, &label).unwrap().source().label(),
        Some(label.as_str()),
    );
    let count = Rc::new(Cell::new(0));
    assert!(matches!(
        LoadedOpening::read_named_json(EndlessReader(count.clone()), &(label + "x")),
        Err(error) if error == "opening_loader_source_label_too_large"
    ));
    assert_eq!(count.get(), 0);
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
    eprintln!("issue120:direct-only:entered-before-assemble bytes={}", bytes.len());
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
    eprintln!("issue120:diagnostic-03:complete hash={} view_bytes={}", observed.0, observed.1.len());
}

#[test]
#[ignore = "isolated fatal-stack diagnostic; run exact with nocapture"]
fn diagnostic_04_loader_bytes_padded() {
    let bytes = diagnostic_bytes(true);
    eprintln!("issue120:diagnostic-04:before-from-json");
    let loaded = LoadedOpening::from_json(&bytes).unwrap();
    eprintln!("issue120:diagnostic-04:loaded-before-drop source_bytes={}", loaded.source().bytes().len());
    drop(loaded);
    eprintln!("issue120:diagnostic-04:complete");
}

#[test]
#[ignore = "isolated fatal-stack diagnostic; run exact with nocapture"]
fn diagnostic_05_loader_reader_padded() {
    let bytes = diagnostic_bytes(true);
    eprintln!("issue120:diagnostic-05:before-read-json");
    let loaded = LoadedOpening::read_json(bytes.as_slice()).unwrap();
    eprintln!("issue120:diagnostic-05:loaded-before-drop source_bytes={}", loaded.source().bytes().len());
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
