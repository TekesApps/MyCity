use std::process::Command;
#[test]
fn real_global_switch_with_live_monitor_realloc_alignment_and_oom_are_finite() {
    let exe = env!("CARGO_BIN_EXE_role-arena-probe");
    let p = Command::new(exe).arg("normal").output().unwrap();
    assert!(p.status.success(), "{}", String::from_utf8_lossy(&p.stderr));
    assert!(String::from_utf8_lossy(&p.stdout).contains("role_arena_process_probe=passed"));
    let p = Command::new(exe).arg("abort").output().unwrap();
    assert!(!p.status.success());
    assert!(
        String::from_utf8_lossy(&p.stderr).contains("memory allocation of 16777216 bytes failed")
    );
}
