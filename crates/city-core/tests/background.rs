use mycity_core::{
    air::State,
    assembly::assemble,
    background::Command as B,
    exchange::Command as X,
    schedule::Phase,
    timeline::Command as T,
    transaction::{Action, Caller, Engine, Envelope, Resource, TxError, ENGINE_BYTE_BUDGET},
};
fn engine() -> Engine {
    Engine::new(
        assemble(include_bytes!("../../../fixtures/m0-exchange-opening.json")).unwrap(),
        [
            (101, 1),
            (102, 1),
            (103, 1),
            (104, 1),
            (201, 2000),
            (202, 2000),
        ]
        .into_iter()
        .map(|(id, cap)| Resource::new(id, cap).unwrap())
        .collect(),
    )
    .unwrap()
}
fn env(e: &Engine, action: Action) -> Envelope {
    Envelope {
        world: 1,
        branch: 1,
        operation_id: e.journal().len() as u64 + 1,
        expected_revision: e.world().view().revision,
        actions: vec![action],
    }
}
fn run(e: &mut Engine, action: Action) {
    e.execute(Caller::System, env(e, action)).unwrap();
}
fn background(command: B) -> Action {
    Action::Background { command }
}
fn advance() -> Action {
    Action::Timeline {
        command: T::Advance {
            target: 30,
            limit: 256,
        },
    }
}
fn ready() -> Engine {
    let mut e = engine();
    run(
        &mut e,
        Action::Exchange {
            command: X::Dispatch { quote: 1 },
        },
    );
    run(&mut e, Action::Timeline { command: T::Enable });
    run(&mut e, background(B::Enable));
    run(&mut e, background(B::Restart));
    for _ in 0..30 {
        run(&mut e, advance());
        if e.timeline().unwrap().progress.phase == Some(Phase::P4) {
            return e;
        }
    }
    panic!("P4 not reached");
}
fn result(e: &Engine) -> State {
    let mut s = e.background().unwrap().frozen_input().unwrap();
    while !s.complete() {
        s.page(8192, 1024).unwrap();
    }
    s
}
fn accept(e: &Engine, s: &State, generation: u32) -> Envelope {
    use mycity_core::sha256;
    let text = sha256(&s.checkpoint().unwrap());
    let mut hash = [0u8; 32];
    for i in 0..32 {
        hash[i] = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).unwrap();
    }
    env(
        e,
        background(B::Accept {
            job: 1,
            generation,
            candidate_sha256: hash,
        }),
    )
}
#[test]
fn p4_requires_complete_candidate_and_commit_hook_remains_the_only_publication_boundary() {
    let mut e = ready();
    let p = e.timeline().unwrap().progress;
    assert_eq!(e.background().unwrap().time(), 0);
    assert_eq!(
        e.execute(Caller::System, env(&e, advance())),
        Err(TxError::BackgroundRequired)
    );
    assert_eq!(e.timeline().unwrap().progress, p);
    run(&mut e, background(B::Lease { job: 1 }));
    let s = result(&e);
    let request = accept(&e, &s, 1);
    let before = e.state_hash();
    let failed =
        e.execute_recorded_with_background(Caller::System, request.clone(), Some(&s), |_, _| {
            Err("io_error")
        });
    assert_eq!(failed, Err("io_error"));
    assert_eq!(e.state_hash(), before);
    let outcome = e
        .execute_recorded_with_background(Caller::System, request.clone(), Some(&s), |_, _| {
            Ok::<(), ()>(())
        })
        .unwrap()
        .unwrap();
    assert_eq!(e.background().unwrap().time(), 30);
    assert!(e.background().unwrap().pending().is_none());
    let hash = e.state_hash();
    assert_eq!(
        e.execute_recorded_with_background(Caller::System, request, Some(&s), |_, _| panic!(
            "duplicate persist"
        ))
        .unwrap_or_else(|_: ()| panic!()),
        Ok(outcome)
    );
    assert_eq!(e.state_hash(), hash);
    run(&mut e, advance());
    assert!(e.timeline().unwrap().progress.target.is_none());
    let restored = Engine::restore_checkpoint(&e.export_checkpoint().unwrap()).unwrap();
    assert_eq!(e.state_hash(), restored.state_hash());
    assert!(restored.owned_capacity_bytes() <= ENGINE_BYTE_BUDGET);
}
#[test]
fn pending_job_survives_checkpoint_and_stale_attempt_cannot_open_the_observation_barrier() {
    let mut e = ready();
    run(&mut e, background(B::Lease { job: 1 }));
    let s = result(&e);
    let bytes = e.export_checkpoint().unwrap();
    let mut e = Engine::restore_checkpoint(&bytes).unwrap();
    run(&mut e, background(B::Restart));
    run(&mut e, background(B::Lease { job: 1 }));
    let stale = accept(&e, &s, 1);
    assert_eq!(
        e.execute_recorded_with_background(
            Caller::System,
            stale,
            Some(&s),
            |_, _| Ok::<(), ()>(())
        )
        .unwrap(),
        Err(TxError::InvalidOperation)
    );
    assert_eq!(e.background().unwrap().time(), 0);
    assert_eq!(e.timeline().unwrap().progress.phase, Some(Phase::P4));
    assert!(e
        .execute_recorded_with_background(
            Caller::Account(1),
            accept(&e, &s, 2),
            Some(&s),
            |_, _| Ok::<(), ()>(())
        )
        .unwrap()
        .is_err());
    let request = accept(&e, &s, 2);
    e.execute_recorded_with_background(Caller::System, request, Some(&s), |_, _| Ok::<(), ()>(()))
        .unwrap()
        .unwrap();
    run(&mut e, advance());
    assert_eq!(e.timeline().unwrap().progress.last_complete, 30);
}

#[test]
fn checkpoint_cannot_erase_a_pending_obligation_or_detach_its_timeline() {
    let e = ready();
    let original: serde_json::Value =
        serde_json::from_slice(&e.export_checkpoint().unwrap()).unwrap();
    for field in ["timeline", "pending", "start", "end", "version"] {
        let mut forged = original.clone();
        match field {
            "timeline" => forged["timeline"]["progress"]["phase"] = serde_json::Value::Null,
            "pending" => forged["background"]["pending"] = serde_json::Value::Null,
            "start" => forged["background"]["pending"]["start"] = serde_json::json!(1),
            "version" => forged["background"]["pending"]["field_version"] = serde_json::json!(1),
            "end" => forged["timeline"]["progress"]["at"] = serde_json::json!(31),
            _ => unreachable!(),
        }
        assert!(
            Engine::restore_checkpoint(&serde_json::to_vec(&forged).unwrap()).is_err(),
            "{field}"
        );
    }
}
