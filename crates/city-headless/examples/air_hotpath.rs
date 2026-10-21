//! Offline, read-only phase diagnostic. Not a throughput or 16GB acceptance gate.
use mycity_core::{
    air::State,
    diagnostics::Trace,
    read_bounded, sha256,
    timeline::Command,
    transaction::{Action, Caller, Engine, Envelope},
};
use serde_json::json;
use std::{fs::File, hint::black_box, time::Instant};
fn measure<T>(name: &str, mut f: impl FnMut() -> T) -> serde_json::Value {
    for _ in 0..5 {
        black_box(f());
    }
    let mut raw = [0u64; 40];
    for ns in &mut raw {
        let start = Instant::now();
        black_box(f());
        *ns = start.elapsed().as_nanos().try_into().expect("bounded time");
    }
    let mut ordered = raw;
    ordered.sort_unstable();
    json!({"name":name,"raw_ns":raw.as_slice(),"p50_ns":ordered[19],"p95_ns":ordered[37],"p99_ns":ordered[39]})
}
fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("immutable Engine image path");
    let bytes = read_bounded(File::open(&path).unwrap(), 4 * 1024 * 1024).unwrap();
    let engine = Engine::restore_checkpoint(&bytes).unwrap();
    let timeline = engine.timeline().unwrap();
    assert_eq!(timeline.claims.len(), 20000);
    assert_eq!(timeline.payroll.len(), 64);
    let field = engine.background().unwrap().field().unwrap();
    assert_eq!(field.input().side, 128);
    let target = timeline.progress.last_complete + 3600;
    let input = field.input().clone();
    let air_bytes = field.checkpoint().unwrap();
    let envelope = Envelope {
        world: engine.world().view().scope.world(),
        branch: engine.world().view().scope.branch(),
        operation_id: engine.world().view().revision + 1,
        expected_revision: engine.world().view().revision,
        actions: vec![Action::Timeline {
            command: Command::Advance {
                target,
                limit: 8192,
            },
        }],
    };
    let mut trace = Trace::enabled();
    let quoted = engine
        .timeline_quote_diagnosed(Caller::System, &envelope, &mut trace)
        .expect("actual valid quote");
    let rows = vec![
        measure("State clone", || field.clone()),
        measure("Input identity", || input.identity()),
        measure("SHA256 checkpoint bytes", || sha256(&air_bytes)),
        measure("State checkpoint digest", || {
            field.checkpoint_digest().unwrap()
        }),
        measure("State checkpoint bytes", || field.checkpoint().unwrap()),
        measure("State restore", || State::restore(&air_bytes).unwrap()),
        measure("Engine timeline quote", || {
            engine.timeline_quote(Caller::System, &envelope).unwrap()
        }),
    ];
    println!("{}",serde_json::to_string_pretty(&json!({
        "schema":"mycity-air-hotpath-diagnostic-v1","source":env!("MYCITY_SOURCE_SHA256"),
        "image_sha256":sha256(&bytes),"path":path,"warmup":5,"samples":40,
        "air_sha256":sha256(&air_bytes),"air_bytes":air_bytes.len(),"source_count":field.input().sources.len(),
        "last_complete":timeline.progress.last_complete,"target":target,"quoted":quoted,"trace":trace.summary,
        "rows":rows,"formal_T13A":"not_accepted; offline diagnostic only"
    })).unwrap());
}
