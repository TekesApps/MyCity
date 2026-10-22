// Fixture-export harness only. Included business/test source remains v119.
include!("construction.rs");

fn copy_closed_fixture_tree(source: &Path, destination: &Path) {
    fs::create_dir(destination).unwrap();
    for item in fs::read_dir(source).unwrap() {
        let item = item.unwrap();
        let kind = item.file_type().unwrap();
        let target = destination.join(item.file_name());
        if kind.is_dir() {
            copy_closed_fixture_tree(&item.path(), &target);
        } else {
            assert!(kind.is_file(), "fixture tree must not contain symlinks or sockets");
            fs::copy(item.path(), target).unwrap();
        }
    }
}

#[test]
fn export_v119_accepted_project_wal_and_checkpoint() {
    let output = PathBuf::from(std::env::var_os("MYCITY_LEGACY_PROJECT_OUT").unwrap());
    assert!(!output.exists(), "refuse to overwrite any fixture or failed run");
    fs::create_dir(&output).unwrap();
    // No Temp/Drop deletion: failed fixture generation keeps its full original.
    let path = output.join("original");
    let mut s = create(&path);
    approvals(&mut s);
    let op = start_id(&s);
    call(&mut s, Caller::Account(4), C::Start, op);
    ordinary_action(&mut s, Caller::System, Action::Timeline { command: T::Enable });
    const SHIFT: u64 = 17 * 3_600;
    advance(&mut s, SHIFT);
    let op = start_id(&s);
    call(&mut s, Caller::Account(3), C::Work {
        at: GameTime::new(SHIFT as i64).unwrap(),
        elapsed_seconds: SHIFT as u32,
        worker_seconds: 28_800,
    }, op);
    let op = start_id(&s);
    call(&mut s, Caller::Account(3), C::SubmitInspection, op);
    let op = start_id(&s);
    call(&mut s, Caller::Account(1), C::AcceptInspection {
        inspector: 1, accepted: true,
    }, op);
    let project = s.engine().unwrap().world().view().construction.unwrap();
    assert!(project.operating_published);
    let wal_hash = s.state_hash().unwrap();
    let wal_sequence = s.committed_sequence();
    drop(s);
    copy_closed_fixture_tree(&path, &output.join("accepted-wal"));

    let mut s = Store::open(&path, RULES).unwrap();
    assert_eq!(s.state_hash().unwrap(), wal_hash);
    s.checkpoint().unwrap();
    let checkpoint_hash = s.state_hash().unwrap();
    let checkpoint_sequence = s.committed_sequence();
    drop(s);
    copy_closed_fixture_tree(&path, &output.join("accepted-checkpoint"));
    let reopened = Store::open(&path, RULES).unwrap();
    assert_eq!(reopened.state_hash().unwrap(), checkpoint_hash);
    drop(reopened);
    fs::write(output.join("report.json"), serde_json::to_vec_pretty(&json!({
        "scope": "new fixtures executed by frozen v119 business implementation plus export-only harness",
        "rules": RULES,
        "project_id": project.id,
        "operating_published": project.operating_published,
        "wal": {"path":"accepted-wal", "state_sha256":wal_hash,"sequence":wal_sequence},
        "checkpoint": {"path":"accepted-checkpoint", "state_sha256":checkpoint_hash,"sequence":checkpoint_sequence},
        "new_candidate_compatibility": "not_run",
        "old_historical_world": false
    })).unwrap()).unwrap();
}
