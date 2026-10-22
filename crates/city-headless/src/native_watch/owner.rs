//! Root-only liveness pipe: observer loss closes the only parent's write end.
use super::err;
use std::{io::Read, time::Instant};
pub fn watch() -> Result<(), String> {
    // An explicit argv marker belongs only to the direct child. Descendants
    // inherit environment but not this marker. Reparenting must never turn a
    // root into an exempt descendant before owner initialization.
    if std::env::args().nth(1).as_deref() != Some("native-observed-root") {
        return Ok(());
    }
    if std::env::args().nth(2).as_deref() == Some("native-watch-fixture")
        && std::env::args().nth(4).as_deref() == Some("delayed-owner")
    {
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    let raw = std::env::var_os("MYCITY_OBSERVER_OWNER_PID").ok_or("native_owner_pid_required")?;
    let parent = raw
        .to_str()
        .ok_or("native_owner_pid_text")?
        .parse::<u32>()
        .map_err(err)?;
    let sample = crate::native_observer::capture(std::process::id(), None, Instant::now())
        .map_err(|e| format!("native_owner_identity:{e:?}"))?;
    if sample.identity.parent_pid != parent {
        return Err("native_owner_parent_changed".into());
    }
    let group = rustix::process::getpgrp();
    if group.as_raw_nonzero().get() as u32 != std::process::id() {
        return Err("native_owner_private_group_required".into());
    }
    let mut token = [0];
    std::io::stdin().read_exact(&mut token).map_err(err)?;
    if token != [b'O'] {
        return Err("native_owner_start_token".into());
    }
    std::thread::Builder::new()
        .name("native-observer-owner".into())
        .stack_size(64 * 1024)
        .spawn(move || {
            let mut b = [0];
            let _ = std::io::stdin().read_exact(&mut b);
            eprintln!("native_observer_owner_lost");
            // This root is the live leader of the private group assigned at
            // spawn. Terminate its children as well; no old/recycled PID is used.
            let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
            std::process::exit(2);
        })
        .map_err(err)?;
    Ok(())
}
