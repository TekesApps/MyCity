use mycity_core::{
    assembly::assemble,
    canonical::Codec,
    transaction::{Action, Engine},
};
use mycity_headless::{
    joint_reference::{self, Reference},
    scale_reference,
};
use serde_json::{json, Value};
use std::io::{self, Write};
fn oracle(e: &Engine) -> (Vec<u8>, Vec<u8>) {
    // Independent old Value implementation from v74, not the borrowed adapters.
    let v: Value = serde_json::from_slice(&e.export_checkpoint().unwrap()).unwrap();
    let mut world = v["world"].clone();
    world["revision"] = json!(0);
    let business=serde_json::to_vec(&json!({"schema":"mycity-business-comparison-v1","world":world,
        "resources":v["resources"],"leases":v["leases"],"next_token":v["next_token"],"timeline":v["timeline"]})).unwrap();
    let state=serde_json::to_vec(&json!({"world":e.world().view(),
        "resources":v["resources"],"leases":v["leases"],"next_token":v["next_token"],"timeline":v["timeline"],"background":v["background"]})).unwrap();
    (business, state)
}
fn check(e: &Engine, codec: &mut Codec) -> usize {
    let (old, hash) = oracle(e);
    let mut actual = Vec::new();
    e.write_business_comparison(codec, &mut actual).unwrap();
    assert_eq!(actual.len(), old.len());
    assert_eq!(actual, old);
    assert_eq!(
        e.business_digest_with(codec).unwrap(),
        mycity_core::sha256(&old)
    );
    let mut state = Vec::new();
    e.write_state_comparison(codec, &mut state).unwrap();
    assert_eq!(state.len(), hash.len());
    assert_eq!(state, hash);
    assert_eq!(
        e.state_hash_with(codec).unwrap(),
        mycity_core::sha256(&hash)
    );
    actual.len()
}
#[test]
fn borrowed_business_and_state_match_old_value_at_full_tables_and_pending_job() {
    let mut codec = Codec::new().unwrap();
    let capacity = codec.owned_capacity_bytes();
    let mut r = Reference::from_unstarted_engine(
        Engine::new(
            assemble(scale_reference::OPENING).unwrap(),
            scale_reference::resources().unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    check(&r.engine, &mut codec);
    let p = serde_json::from_slice(scale_reference::POLICY).unwrap();
    for a in scale_reference::startup_actions(&p).unwrap() {
        r.execute(vec![a]).unwrap();
    }
    assert_eq!(r.engine.timeline().unwrap().claims.len(), 20000);
    assert_eq!(r.engine.timeline().unwrap().payroll.len(), 64);
    let bytes = check(&r.engine, &mut codec);
    assert!(
        r.comparison_capacity_bytes()
            <= Codec::LIMIT
                + mycity_core::canonical::Workspace::backing_bytes()
                + std::mem::size_of::<Codec>()
                + 256
    );
    assert!(bytes > 1_000_000 && bytes < Codec::LIMIT);
    // Advance schedules a real pending job; do not run worker/accept as a substitute.
    use mycity_core::timeline::Command as T;
    for _ in 0..10 {
        r.execute(vec![Action::Timeline {
            command: T::Advance {
                target: 60,
                limit: 256,
            },
        }])
        .unwrap();
        if r.engine.background().unwrap().pending().is_some() {
            break;
        }
    }
    assert!(r.engine.background().unwrap().pending().is_some());
    check(&r.engine, &mut codec);
    r.compare_business(&r.engine).unwrap();
    assert_eq!(codec.owned_capacity_bytes(), capacity);
}
#[test]
fn writer_failure_and_input_overflow_leave_same_codec_reusable() {
    struct Fail(usize);
    impl Write for Fail {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            if self.0 == 0 {
                return Err(io::Error::other("injected"));
            }
            let n = self.0.min(b.len());
            self.0 -= n;
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let e = joint_reference::engine().unwrap();
    let mut codec = Codec::new().unwrap();
    let before = codec.owned_capacity_bytes();
    for n in [0, 1, 127, 8192] {
        assert!(e.write_business_comparison(&mut codec, Fail(n)).is_err());
        check(&e, &mut codec);
    }
    assert!(codec.write(&vec![0u8; Codec::LIMIT], io::sink()).is_err());
    check(&e, &mut codec);
    assert_eq!(codec.owned_capacity_bytes(), before);
}
