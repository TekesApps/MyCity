//! Source-only unit checks for `normal_pool::startup_owner`.
//! Root may include this beside the owner after its serial integration window;
//! no test or build was run for this candidate.

use super::startup_owner::StartupOwner;
use std::process::Command;

#[test]
fn new_owner_is_empty_and_child_access_is_checked() {
    let mut owner = StartupOwner::new();
    assert!(owner.is_empty());
    assert!(owner.child().is_err());
    assert!(owner.child_mut().is_err());
    assert!(owner.bind_owned_child().is_err());
    assert_eq!(owner.pid_opt().unwrap(), None);
    assert_eq!(owner.real_exit_status().unwrap(), None);
    assert!(owner.retire().is_ok());
    assert!(owner.is_empty());
}

#[test]
fn no_pool_configuration_does_not_spawn_or_claim_a_child() {
    let owner = StartupOwner::new();
    let mut command = Command::new("/bin/true");

    // configure only mutates a command when an existing Supervisor owns it.
    assert!(owner.configure(&mut command).is_ok());
    assert_eq!(owner.pid_opt().unwrap(), None);
    assert!(owner.is_empty());
}

#[test]
fn retained_owner_layout_is_reported_without_native_memory_claim() {
    use super::broker::{PendingRegistration, Supervisor};
    use std::mem::size_of;
    println!("startup_owner_bytes={} pending_registration_bytes={} supervisor_bytes={} child_bytes={}",
        size_of::<StartupOwner>(), size_of::<PendingRegistration>(), size_of::<Supervisor>(), size_of::<std::process::Child>());
}
