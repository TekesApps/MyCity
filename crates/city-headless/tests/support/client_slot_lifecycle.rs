//! ClientSlot retirement regressions. Root includes this module from broker.rs
//! so the tests reuse its real Temp, FaultPool, exe, call, and status fixtures.
//! Source-only candidate: none of these tests were run here.
use super::*;
use mycity_headless::sim_client::{with_client_scope, StartupFault};
use std::process::Command as HostCommand;

#[test]
fn partial_startup_cuts_retain_slot_across_lost_revoke_until_ack() {
    for (label, fault) in [
        ("missing_stdout", StartupFault::MissingStdout),
        ("reader_rejected", StartupFault::ReaderRejected),
    ] {
        let t = Temp::new();
        let root = t.0.join("pool");
        let mut pool = FaultPool::spawn(&root, "after-broker-revoke-reply");
        let world = t.0.join("world");
        let mut client = Client::new();

        assert!(client
            .start_in_pool_with_fault(exe(), &world, true, &root, fault)
            .is_err(), "{label}");
        assert!(!client.is_empty(), "{label}: partial startup must retain owner");
        assert!(client.stop().is_err(), "{label}: lost Revoke reply is unresolved");
        assert!(!client.is_empty(), "{label}: failed retirement must retain slot");
        assert!(client
            .start_in_pool(exe(), &t.0.join("replacement"), true, false, &root)
            .is_err(), "{label}: a replacement must be rejected");
        client.stop().unwrap();
        assert!(client.is_empty(), "{label}: slot clears only after retry ack");

        let done = normal_pool::status(&root).unwrap();
        assert_eq!(done.workspace_grants, done.workspace_returns, "{label}");
        assert_eq!(done.available_touched_bytes, BYTES, "{label}");
        pool.finish();
    }
}

#[test]
fn client_slot_retries_lost_register_with_same_request_then_cancels_once() {
    let t = Temp::new();
    let root = t.0.join("pool");
    let mut pool = FaultPool::spawn(&root, "after-broker-register-reply");
    let mut client = Client::new();

    assert!(client
        .start_in_pool(exe(), &t.0.join("world"), true, false, &root)
        .is_err());
    assert!(!client.is_empty(), "lost Register reply must retain pending owner");
    let unknown = normal_pool::status(&root).unwrap();
    assert_eq!(unknown.workspace_grants, 1);
    assert_eq!(unknown.workspace_returns, 0);

    client.stop().unwrap();
    assert!(client.is_empty());
    let done = normal_pool::status(&root).unwrap();
    assert_eq!(done.workspace_grants, 1);
    assert_eq!(done.workspace_returns, 1);
    assert_eq!(done.available_touched_bytes, BYTES);
    pool.finish();
}

fn start_ready_client(client: &mut Client, world: &Path, root: &Path) {
    client.start_in_pool(exe(), world, true, false, root).unwrap();
    assert_eq!(client.next(Duration::from_secs(10)).unwrap().kind, "ready");
}

#[test]
fn actual_wait_and_retirement_are_repeatable_and_fresh_start_resets_terminal_state() {
    let t = Temp::new();
    let root = t.0.join("pool");
    let mut pool = Process::spawn(exe(), &root, None).unwrap();
    let mut client = Client::new();

    assert!(client.wait_for_sim_exit(Duration::from_millis(10)).is_err());
    start_ready_client(&mut client, &t.0.join("world"), &root);
    let mut id = 0;
    call(&mut client, &mut id, Command::Stop);
    client.wait_for_sim_exit(Duration::from_secs(10)).unwrap();
    client.wait_for_sim_exit(Duration::from_secs(10)).unwrap();
    client.wait_for_sim_exit(Duration::from_secs(10)).unwrap();
    client.stop().unwrap();

    assert!(client
        .start_in_pool(exe(), &PathBuf::from("relative-world"), true, false, &root)
        .is_err());
    assert!(client.wait_for_sim_exit(Duration::from_millis(10)).is_err(),
        "failed fresh start must not inherit the old retired flag");
    assert!(client.is_empty());
    let done = normal_pool::status(&root).unwrap();
    assert_eq!(done.workspace_grants, done.workspace_returns);
    assert_eq!(done.available_touched_bytes, BYTES);
    pool.finish().unwrap();
}

#[test]
fn client_scope_cleans_up_before_returning_body_error_or_resuming_panic() {
    let t = Temp::new();
    let root = t.0.join("pool");
    let mut pool = Process::spawn(exe(), &root, None).unwrap();
    let world = t.0.join("world");
    let body_result = with_client_scope(|client| {
        start_ready_client(client, &world, &root);
        Err::<(), _>("original_body_error".into())
    });
    assert_eq!(body_result.unwrap_err(), "original_body_error");
    let after_error = normal_pool::status(&root).unwrap();
    assert_eq!(after_error.workspace_grants, after_error.workspace_returns);
    assert_eq!(after_error.available_touched_bytes, BYTES);

    let panic_world = t.0.join("panic-world");
    let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_client_scope::<()>(|client| {
            start_ready_client(client, &panic_world, &root);
            std::panic::panic_any("original_scope_panic");
        })
    }));
    let payload = panic_result.unwrap_err();
    assert_eq!(payload.downcast_ref::<&'static str>(), Some(&"original_scope_panic"));
    let after_panic = normal_pool::status(&root).unwrap();
    assert_eq!(after_panic.workspace_grants, after_panic.workspace_returns);
    assert_eq!(after_panic.available_touched_bytes, BYTES);
    pool.finish().unwrap();
}

/// Child-process helper used only by the terminal-scope parent regression.
/// The environment values are supplied by that parent after it owns the pool.
#[test]
#[ignore = "subprocess helper; invoked only by terminal scope regression"]
fn terminal_scope_child_helper() {
    let root = PathBuf::from(
        std::env::var_os("MYCITY_SLOT_SCOPE_POOL_ROOT")
            .expect("parent must provide its fixture pool root"),
    );
    let expected_pool_pid: u32 = std::env::var("MYCITY_SLOT_SCOPE_POOL_PID")
        .expect("parent must provide its original pool PID")
        .parse()
        .expect("pool PID must be numeric");
    let sim_pid_file = PathBuf::from(
        std::env::var_os("MYCITY_SLOT_SCOPE_SIM_PID_FILE")
            .expect("parent must provide a SIM PID evidence path"),
    );

    let ready: Value = serde_json::from_slice(
        &fs::read(root.join("ready.json")).expect("parent-owned pool readiness evidence"),
    )
    .expect("pool ready evidence JSON");
    assert_eq!(ready["pid"].as_u64(), Some(u64::from(expected_pool_pid)));

    let result = with_client_scope(|client| {
        start_ready_client(client, &root.join("terminal-world"), &root);
        fs::write(&sim_pid_file, client.pid().to_string())
            .expect("record actual owned Sim PID before broker failure");

        // Kill only the exact pool process the parent started and whose ready
        // receipt was checked above. This is test-fixture cleanup failure.
        let status = HostCommand::new("kill")
            .args(["-KILL", &expected_pool_pid.to_string()])
            .status()
            .expect("signal the exact parent-owned pool fixture");
        assert!(status.success(), "original fixture pool kill failed");
        Err::<(), _>("deliberate_body_error_after_pool_loss".into())
    });
    panic!("terminal scope returned instead of exiting 2: {result:?}");
}

#[test]
fn client_scope_exits_two_with_unresolved_receipt_when_original_pool_is_lost() {
    let t = Temp::new();
    let root = t.0.join("pool");
    // This fault is not reached because the helper performs no worker spawn;
    // FaultPool exposes its actual child PID to the subprocess helper.
    let mut pool = FaultPool::spawn(&root, "after-broker-spawn-pause");
    let sim_pid_file = t.0.join("terminal-sim-pid.txt");

    let mut child = HostCommand::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "client_slot_lifecycle::terminal_scope_child_helper",
            "--ignored",
            "--nocapture",
        ])
        .env("MYCITY_SLOT_SCOPE_POOL_ROOT", &root)
        .env("MYCITY_SLOT_SCOPE_POOL_PID", pool.child.id().to_string())
        .env("MYCITY_SLOT_SCOPE_SIM_PID_FILE", &sim_pid_file)
        .stdout(fs::File::create_new(t.0.join("terminal-helper-stdout.log")).unwrap())
        .stderr(fs::File::create_new(t.0.join("terminal-helper-stderr.log")).unwrap())
        .spawn()
        .unwrap();

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let waited = child.wait();
            fs::write(t.0.join("terminal-helper-timeout.txt"), format!("{waited:?}\n")).unwrap();
            panic!("terminal-scope subprocess exceeded 10 seconds; owned helper reaped");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let helper_pid = child.id();
    child.wait().unwrap();
    let stderr_bytes = mycity_core::read_bounded(fs::File::open(t.0.join("terminal-helper-stderr.log")).unwrap(), 64 * 1024).unwrap();
    fs::write(t.0.join("terminal-helper-status.txt"), format!("{status:?}\n")).unwrap();
    assert_eq!(status.code(), Some(2));
    let sim_pid: u32 = fs::read_to_string(&sim_pid_file)
        .expect("helper must record its actual Sim PID")
        .parse()
        .unwrap();
    assert!(sim_pid > 0);

    let stderr = String::from_utf8(stderr_bytes).unwrap();
    let receipt = stderr
        .lines()
        .find_map(|line| serde_json::from_str::<Value>(line).ok())
        .expect("terminal cleanup must emit its machine-readable unresolved receipt");
    assert_eq!(receipt["schema"], "mycity-unresolved-owner-v1");
    assert_eq!(receipt["scope"], "sim-client");
    assert_eq!(receipt["pid"], helper_pid);
    assert_eq!(receipt["owner"]["pid"], sim_pid);
    assert_eq!(receipt["owner"]["actual_wait"], true, "real wait precedes failed broker retirement");
    assert!(receipt["owner"]["exit_success"].is_boolean());
    assert_eq!(receipt["retired"], false);
    assert_eq!(receipt["new_start_forbidden"], true);
    assert_eq!(
        receipt["body_error"],
        "deliberate_body_error_after_pool_loss"
    );
    assert!(pool.child.try_wait().unwrap().is_some(),
        "only the original fixture pool should have been killed");
    assert!(!root.join("report.json").exists(),
        "a killed pool must not be represented as a clean report");
    // FaultPool::drop reaps the already-killed original fixture process.
}

#[test]
fn definitive_busy_rejection_does_not_discard_or_terminate_the_original_client() {
    let t = Temp::new();
    let root = t.0.join("pool");
    let mut pool = Process::spawn(exe(), &root, None).unwrap();
    let mut first = Client::new();
    start_ready_client(&mut first, &t.0.join("original-world"), &root);
    let result = with_client_scope(|second| second.start_in_pool(exe(), &t.0.join("rejected-world"), true, false, &root));
    assert_eq!(result.unwrap_err(), "normal_broker_session_busy");
    let unchanged = normal_pool::status(&root).unwrap();
    assert_eq!(unchanged.workspace_grants, 1);
    assert_eq!(unchanged.workspace_returns, 0);
    let mut id = 0;
    let state = call(&mut first, &mut id, Command::Status);
    assert!(state.get("sequence").is_some());
    first.stop().unwrap();
    let done = normal_pool::status(&root).unwrap();
    assert_eq!(done.workspace_grants, done.workspace_returns);
    pool.finish().unwrap();
}
