use mycity_core::{
    background::Command as B,
    exchange::Command as X,
    schedule::Phase,
    timeline::Command as T,
    transaction::{Action, Envelope},
};
use mycity_headless::{sim_client::Client, sim_host::Command};
use serde_json::Value;
use std::{path::Path, time::Duration};
fn call(client: &mut Client, id: &mut u64, command: Command) -> Value {
    *id += 1;
    client.submit(*id, command).unwrap();
    for _ in 0..24 {
        let event = client.next(Duration::from_secs(10)).unwrap();
        if event.kind == "result" {
            assert!(event.payload.get("Err").is_none(), "{:?}", event);
            return event.payload["Ok"].clone();
        }
    }
    panic!("bounded result missing")
}
fn action(client: &mut Client, id: &mut u64, a: Action) {
    let s = call(client, id, Command::Status);
    let envelope = Envelope {
        world: 1,
        branch: 1,
        operation_id: s["sequence"].as_u64().unwrap() + 1,
        expected_revision: s["revision"].as_u64().unwrap(),
        actions: vec![a],
    };
    let result = call(client, id, Command::Execute { envelope });
    assert!(result["model"].get("Ok").is_some(), "{result}");
}
#[test]
fn independent_sim_owns_lock_runs_worker_and_returns_original_cold_receipt_after_reopen() {
    let dir = std::env::temp_dir().join(format!("mycity-independent-sim-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let world = dir.join("world");
    let executable = Path::new(env!("CARGO_BIN_EXE_mycity-headless"));
    let mut client = Client::spawn(executable, &world, true).unwrap();
    assert_eq!(client.next(Duration::from_secs(10)).unwrap().kind, "ready");
    assert_ne!(client.pid(), std::process::id());
    assert!(matches!(
        mycity_storage::Store::open(&world, "irrelevant"),
        Err(mycity_storage::Error::Locked)
    ));
    let mut id = 0;
    action(
        &mut client,
        &mut id,
        Action::Exchange {
            command: X::Dispatch { quote: 1 },
        },
    );
    action(
        &mut client,
        &mut id,
        Action::Timeline { command: T::Enable },
    );
    action(
        &mut client,
        &mut id,
        Action::Background { command: B::Enable },
    );
    for _ in 0..30 {
        action(
            &mut client,
            &mut id,
            Action::Timeline {
                command: T::Advance {
                    target: 30,
                    limit: 256,
                },
            },
        );
        let s = call(&mut client, &mut id, Command::Status);
        if serde_json::from_value::<Option<Phase>>(s["timeline"]["phase"].clone()).unwrap()
            == Some(Phase::P4)
        {
            break;
        }
    }
    let first = call(&mut client, &mut id, Command::RunRequired { job: 1 });
    assert_eq!(first["replayed"], false);
    let s = call(&mut client, &mut id, Command::Status);
    assert_eq!(s["background"]["time"], 30);
    assert_eq!(s["capacity"]["worker_cache_charged_bytes"], 6 * 1024 * 1024);
    assert!(s["capacity"]["worker_cache_actual_bytes"].as_u64().unwrap() <= 6 * 1024 * 1024);
    call(&mut client, &mut id, Command::Checkpoint);
    call(&mut client, &mut id, Command::Stop);
    client.stop().unwrap();
    let mut reopened = Client::spawn(executable, &world, false).unwrap();
    assert_eq!(
        reopened.next(Duration::from_secs(10)).unwrap().kind,
        "ready"
    );
    let again = call(&mut reopened, &mut id, Command::RunRequired { job: 1 });
    assert_eq!(again["replayed"], true);
    assert_eq!(first["record"], again["record"]);
    call(&mut reopened, &mut id, Command::Stop);
    reopened.stop().unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

fn prepare(client: &mut Client, id: &mut u64) {
    action(
        client,
        id,
        Action::Exchange {
            command: X::Dispatch { quote: 2 },
        },
    );
    action(
        client,
        id,
        Action::Exchange {
            command: X::Dispatch { quote: 1 },
        },
    );
    action(client, id, Action::Timeline { command: T::Enable });
    action(client, id, Action::Background { command: B::Enable });
    for _ in 0..30 {
        action(
            client,
            id,
            Action::Timeline {
                command: T::Advance {
                    target: 30,
                    limit: 256,
                },
            },
        );
        let state = call(client, id, Command::Status);
        if serde_json::from_value::<Option<Phase>>(state["timeline"]["phase"].clone()).unwrap()
            == Some(Phase::P4)
        {
            return;
        }
    }
    panic!("P4 absent");
}
fn barrier(client: &mut Client) -> Value {
    for _ in 0..8 {
        let event = client.next(Duration::from_secs(10)).unwrap();
        if event.kind == "probe_barrier" {
            return event.payload;
        }
        assert_ne!(event.kind, "result", "probe never reached");
    }
    panic!("barrier absent");
}
#[test]
fn actual_worker_sigkill_in_sources_and_cells_retries_in_same_sim_with_new_generation() {
    use mycity_headless::sim_host::ProbePoint;
    let exe = Path::new(env!("CARGO_BIN_EXE_mycity-headless"));
    for (index, point) in [ProbePoint::WorkerSources, ProbePoint::WorkerCells]
        .into_iter()
        .enumerate()
    {
        let root = std::env::temp_dir().join(format!(
            "mycity-sim-worker-kill-{}-{index}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        let world = root.join("world");
        let mut client = Client::spawn_probe(exe, &world, true).unwrap();
        client.next(Duration::from_secs(10)).unwrap();
        let mut id = 0;
        prepare(&mut client, &mut id);
        id += 1;
        client
            .submit(id, Command::ProbeRequired { job: 1, point })
            .unwrap();
        let at = barrier(&mut client);
        assert!(at["details"]["cursor"].as_u64().unwrap() > 0);
        let old = client.worker_pid().unwrap();
        client.kill_owned_worker().unwrap();
        client.acknowledge_probe(point, false).unwrap();
        let mut failed = 0;
        let mut replacement = None;
        for _ in 0..16 {
            let event = client.next(Duration::from_secs(10)).unwrap();
            match event.kind.as_str() {
                "attempt_failed" => failed += 1,
                "worker_started" => replacement = event.payload["worker_pid"].as_u64(),
                "result" => {
                    assert!(event.payload.get("Ok").is_some(), "{event:?}");
                    break;
                }
                _ => (),
            }
        }
        assert_eq!(failed, 1);
        assert_ne!(replacement.unwrap(), u64::from(old));
        let state = call(&mut client, &mut id, Command::Status);
        assert_eq!(state["background"]["version"], 1);
        assert_eq!(state["background"]["time"], 30);
        assert_eq!(state["background"]["incarnation"], 2);
        call(&mut client, &mut id, Command::Stop);
        client.stop().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn actual_sim_sigkill_at_lease_uncommitted_asset_and_committed_reply_recovers() {
    use mycity_headless::sim_host::ProbePoint;
    let exe = Path::new(env!("CARGO_BIN_EXE_mycity-headless"));
    let mut reference = None;
    for (index, point) in [
        ProbePoint::WorkerCells,
        ProbePoint::Leased,
        ProbePoint::CandidatePublished,
        ProbePoint::Committed,
    ]
    .into_iter()
    .enumerate()
    {
        let root =
            std::env::temp_dir().join(format!("mycity-sim-kill-{}-{index}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let world = root.join("world");
        let mut client = Client::spawn_probe(exe, &world, true).unwrap();
        client.next(Duration::from_secs(10)).unwrap();
        let mut id = 0;
        prepare(&mut client, &mut id);
        id += 1;
        client
            .submit(id, Command::ProbeRequired { job: 1, point })
            .unwrap();
        barrier(&mut client);
        // External SIGKILL: the old Client does NOT clean its worker before reopen.
        assert!(std::process::Command::new("kill")
            .args(["-KILL", &client.pid().to_string()])
            .status()
            .unwrap()
            .success());
        client.wait_for_sim_exit(Duration::from_secs(2)).unwrap();
        let mut recovered = Client::spawn(exe, &world, false).unwrap();
        let ready = recovered.next(Duration::from_secs(10)).unwrap();
        if point == ProbePoint::Committed {
            assert_eq!(ready.payload["background"]["version"], 1);
        } else {
            assert_eq!(ready.payload["background"]["version"], 0);
            assert_eq!(ready.payload["background"]["pending"]["attempt"], 1);
        }
        let result = call(&mut recovered, &mut id, Command::RunRequired { job: 1 });
        assert_eq!(result["replayed"], point == ProbePoint::Committed);
        let state = call(&mut recovered, &mut id, Command::Status);
        assert_eq!(state["background"]["time"], 30);
        if let Some(expected) = &reference {
            assert_eq!(&state["background"]["field"], expected);
        } else {
            reference = Some(state["background"]["field"].clone());
        }
        // Root already committed means no second lease; pre-commit crash needs one.
        assert_eq!(
            state["background"]["incarnation"],
            if point == ProbePoint::Committed { 1 } else { 2 }
        );
        call(&mut recovered, &mut id, Command::Stop);
        recovered.stop().unwrap();
        client.stop().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn cancelling_attempt_keeps_required_barrier_and_manual_retry_completes() {
    use mycity_headless::sim_host::ProbePoint;
    let exe = Path::new(env!("CARGO_BIN_EXE_mycity-headless"));
    let root = std::env::temp_dir().join(format!("mycity-sim-cancel-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let mut client = Client::spawn_probe(exe, &root.join("world"), true).unwrap();
    client.next(Duration::from_secs(10)).unwrap();
    let mut id = 0;
    prepare(&mut client, &mut id);
    id += 1;
    client
        .submit(
            id,
            Command::ProbeRequired {
                job: 1,
                point: ProbePoint::WorkerCells,
            },
        )
        .unwrap();
    barrier(&mut client);
    client
        .acknowledge_probe(ProbePoint::WorkerCells, true)
        .unwrap();
    loop {
        let event = client.next(Duration::from_secs(10)).unwrap();
        if event.kind == "result" {
            assert!(event.payload.get("Err").is_some());
            break;
        }
    }
    let state = call(&mut client, &mut id, Command::Status);
    assert_eq!(state["background"]["pending"]["failure"], "cancelled");
    assert_eq!(state["background"]["time"], 0);
    assert_eq!(
        state["timeline"]["phase"],
        serde_json::to_value(Phase::P4).unwrap()
    );
    call(&mut client, &mut id, Command::RunRequired { job: 1 });
    let state = call(&mut client, &mut id, Command::Status);
    assert_eq!(state["background"]["time"], 30);
    call(&mut client, &mut id, Command::Stop);
    client.stop().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn internal_control_namespace_cannot_be_preoccupied_by_execute() {
    let exe = Path::new(env!("CARGO_BIN_EXE_mycity-headless"));
    let root = std::env::temp_dir().join(format!("mycity-sim-namespace-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let mut client = Client::spawn(exe, &root.join("world"), true).unwrap();
    client.next(Duration::from_secs(10)).unwrap();
    let mut id = 0;
    prepare(&mut client, &mut id);
    let state = call(&mut client, &mut id, Command::Status);
    let envelope = Envelope {
        world: 1,
        branch: 1,
        operation_id: 0xd000_0000_0000_0000 | (state["sequence"].as_u64().unwrap() + 2),
        expected_revision: state["revision"].as_u64().unwrap(),
        actions: vec![Action::Timeline {
            command: T::Advance {
                target: 30,
                limit: 256,
            },
        }],
    };
    id += 1;
    client.submit(id, Command::Execute { envelope }).unwrap();
    let event = client.next(Duration::from_secs(10)).unwrap();
    assert_eq!(event.payload["Err"], "sim_internal_control_reserved");
    let after = call(&mut client, &mut id, Command::Status);
    assert_eq!(state["sequence"], after["sequence"]);
    call(&mut client, &mut id, Command::RunRequired { job: 1 });
    call(&mut client, &mut id, Command::Stop);
    client.stop().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn three_real_cancelled_workers_exhaust_attempts_without_erasing_required_job() {
    use mycity_headless::sim_host::ProbePoint;
    let exe = Path::new(env!("CARGO_BIN_EXE_mycity-headless"));
    let root = std::env::temp_dir().join(format!("mycity-sim-exhausted-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let mut client = Client::spawn_probe(exe, &root.join("world"), true).unwrap();
    client.next(Duration::from_secs(10)).unwrap();
    let mut id = 0;
    prepare(&mut client, &mut id);
    for attempt in 1..=3 {
        id += 1;
        client
            .submit(
                id,
                Command::ProbeRequired {
                    job: 1,
                    point: ProbePoint::WorkerCells,
                },
            )
            .unwrap();
        barrier(&mut client);
        client
            .acknowledge_probe(ProbePoint::WorkerCells, true)
            .unwrap();
        loop {
            let event = client.next(Duration::from_secs(10)).unwrap();
            if event.kind == "result" {
                assert!(event.payload.get("Err").is_some());
                break;
            }
        }
        let state = call(&mut client, &mut id, Command::Status);
        assert_eq!(state["background"]["pending"]["attempt"], attempt);
        assert_eq!(state["background"]["time"], 0);
    }
    id += 1;
    client.submit(id, Command::RunRequired { job: 1 }).unwrap();
    let event = client.next(Duration::from_secs(10)).unwrap();
    assert_eq!(event.payload["Err"], "sim_retry_exhausted_or_wrong_job");
    call(&mut client, &mut id, Command::Stop);
    client.stop().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
