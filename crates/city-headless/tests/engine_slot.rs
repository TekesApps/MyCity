use mycity_core::{
    assembly::assemble,
    timeline::Command as T,
    transaction::{Action, Engine, EngineSlot},
};
use mycity_headless::{joint_reference::Reference, scale_reference};
use std::io::{self, Read};
fn full() -> Engine {
    let mut r = Reference::from_unstarted_engine(
        Engine::new(
            assemble(scale_reference::OPENING).unwrap(),
            scale_reference::resources().unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let p = serde_json::from_slice(scale_reference::POLICY).unwrap();
    for a in scale_reference::startup_actions(&p).unwrap() {
        r.execute(vec![a]).unwrap();
    }
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
    assert_eq!(r.engine.timeline().unwrap().claims.len(), 20000);
    assert!(r.engine.background().unwrap().pending().is_some());
    r.engine
}
#[test]
fn full_reader_restore_copy_null_failure_and_reset_retain_every_backing() {
    let e = full();
    let bytes = e.export_checkpoint().unwrap();
    let mut slot = EngineSlot::new().unwrap();
    let identity = slot.identity().unwrap();
    assert!(slot.engine().is_err());
    struct Fragments<'a>(&'a [u8]);
    impl Read for Fragments<'_> {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            let n = b.len().min(13).min(self.0.len());
            b[..n].copy_from_slice(&self.0[..n]);
            self.0 = &self.0[n..];
            Ok(n)
        }
    }
    slot.restore_reader_into(Fragments(&bytes)).unwrap();
    assert_eq!(slot.engine().unwrap().export_checkpoint().unwrap(), bytes);
    assert_eq!(slot.identity().unwrap(), identity);
    let small = Engine::new(
        assemble(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap(),
        vec![],
    )
    .unwrap();
    for source in [&small, &e, &small, &e] {
        slot.copy_from_into(source).unwrap();
        assert_eq!(
            slot.engine().unwrap().export_checkpoint().unwrap(),
            source.export_checkpoint().unwrap()
        );
        assert_eq!(slot.identity().unwrap(), identity);
    }
    for n in [0, 1, 64, bytes.len() / 2, bytes.len() - 1] {
        assert!(slot.restore_into(&bytes[..n]).is_err());
        assert!(slot.engine().is_err());
        assert_eq!(slot.identity().unwrap(), identity);
        slot.restore_into(&bytes).unwrap();
    }
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let extra = v["timeline"]["claims"][0].clone();
    v["timeline"]["claims"].as_array_mut().unwrap().push(extra);
    assert!(slot.restore_into(&serde_json::to_vec(&v).unwrap()).is_err());
    assert_eq!(slot.identity().unwrap(), identity);
    slot.restore_into(&bytes).unwrap();
    slot.reset_preserving_capacity();
    assert!(slot.engine().is_err());
    assert_eq!(slot.identity().unwrap(), identity);
    slot.copy_from_into(&e).unwrap();
    assert_eq!(slot.engine().unwrap().export_checkpoint().unwrap(), bytes);
}
#[test]
fn failed_candidate_never_replaces_active_root_and_swap_only_moves_existing_backing() {
    let mut active = full();
    let before = active.export_checkpoint().unwrap();
    let mut slot = EngineSlot::new().unwrap();
    let empty_id = slot.identity().unwrap();
    assert!(slot.restore_into(b"{\"world\":null}").is_err());
    assert!(slot.publish_swap(&mut active).is_err());
    assert_eq!(active.export_checkpoint().unwrap(), before);
    slot.copy_from_into(&active).unwrap();
    slot.publish_swap(&mut active).unwrap();
    assert_eq!(active.export_checkpoint().unwrap(), before);
    assert!(slot.engine().is_err());
    assert_ne!(slot.identity().unwrap().world, empty_id.world);
    slot.copy_from_into(&active).unwrap();
    slot.publish_swap(&mut active).unwrap();
    assert_eq!(slot.identity().unwrap(), empty_id);
    assert_eq!(active.export_checkpoint().unwrap(), before);
}

#[test]
fn optional_world_fields_default_after_reuse_and_duplicate_or_unknown_fields_stay_unpublished() {
    let mut slot = EngineSlot::new().unwrap();
    slot.copy_from_into(&full()).unwrap();
    let identity = slot.identity().unwrap();
    let small = Engine::new(
        assemble(include_bytes!("../../../fixtures/m0-e0-opening.json")).unwrap(),
        vec![],
    )
    .unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(&small.export_checkpoint().unwrap()).unwrap();
    for name in ["region", "economy", "boot", "exchange"] {
        value["world"].as_object_mut().unwrap().remove(name);
    }
    let bytes = serde_json::to_vec(&value).unwrap();
    let normal = Engine::restore_checkpoint(&bytes).unwrap();
    slot.restore_into(&bytes).unwrap();
    assert_eq!(
        slot.engine().unwrap().export_checkpoint().unwrap(),
        normal.export_checkpoint().unwrap()
    );
    assert_eq!(slot.identity().unwrap(), identity);
    for bad in [
        format!(
            "{{\"next_token\":1,{}",
            std::str::from_utf8(&bytes[1..]).unwrap()
        ),
        format!(
            "{{\"unknown\":null,{}",
            std::str::from_utf8(&bytes[1..]).unwrap()
        ),
    ] {
        assert!(slot.restore_into(bad.as_bytes()).is_err());
        assert!(slot.engine().is_err());
        assert_eq!(slot.identity().unwrap(), identity);
    }
    slot.restore_into(&bytes).unwrap();
    assert_eq!(slot.identity().unwrap(), identity);
}
