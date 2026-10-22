//! Bounded descendant discovery plus an authenticated lifecycle journal.
use super::registration::{Kind, Record};
use crate::{
    control_data::Text,
    native_observer::{capture, Identity, Sample, Unknown},
};
use serde::Serialize;
use std::time::Instant;
const CAP: usize = 512;
#[derive(Clone, Copy)]
struct Tracked {
    identity: Identity,
    role: Text<48>,
    seen: bool,
    registered: bool,
}
#[derive(Clone, Copy, Serialize)]
pub(super) struct Member {
    pub sample: Sample,
    pub role: Text<48>,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct Issue {
    pub pid: u32,
    pub expected_birth: Option<u64>,
    pub kind: &'static str,
    pub read_error: Option<Unknown>,
}
pub(super) struct Tracker {
    known: Vec<Tracked>,
    pub members: Vec<Member>,
    pub issues: Vec<Issue>,
    pub identities: u64,
    pub retired: u64,
    pub unpolled_registered: u64,
    pub max_live: usize,
}
impl Tracker {
    pub fn new(own: Sample) -> Result<Self, String> {
        let mut s = Self {
            known: Vec::with_capacity(CAP),
            members: Vec::with_capacity(CAP),
            issues: Vec::with_capacity(CAP),
            identities: 0,
            retired: 0,
            unpolled_registered: 0,
            max_live: 0,
        };
        s.add(own.identity, Text::new("observer")?, false)?;
        Ok(s)
    }
    pub(super) fn add(
        &mut self,
        identity: Identity,
        role: Text<48>,
        registered: bool,
    ) -> Result<(), String> {
        if let Some(x) = self.known.iter_mut().find(|x| {
            x.identity.pid == identity.pid && x.identity.start_abstime == identity.start_abstime
        }) {
            if registered {
                x.registered = true;
                x.role = role;
            }
            return Ok(());
        }
        if self.known.len() == CAP {
            return Err("native_observer_identity_capacity".into());
        }
        self.known.push(Tracked {
            identity,
            role,
            seen: false,
            registered,
        });
        self.identities += 1;
        self.max_live = self.max_live.max(self.known.len());
        Ok(())
    }
    pub fn register(&mut self, r: Record) -> Result<(), String> {
        if r.kind == Kind::Started {
            self.add(r.sample.identity, r.role, true)?;
        }
        Ok(())
    }
    fn issue(
        &mut self,
        entry: Tracked,
        kind: &'static str,
        read_error: Option<Unknown>,
    ) -> Result<(), String> {
        if self.issues.len() == CAP {
            return Err("native_observer_issue_capacity".into());
        }
        self.issues.push(Issue {
            pid: entry.identity.pid,
            expected_birth: Some(entry.identity.start_abstime),
            kind,
            read_error,
        });
        Ok(())
    }
    pub fn poll(&mut self, origin: Instant) -> Result<(), String> {
        self.members.clear();
        self.issues.clear();
        let mut i = 0;
        while i < self.known.len() {
            let entry = self.known[i];
            match capture(entry.identity.pid, None, origin) {
                Ok(sample) if sample.identity.start_abstime == entry.identity.start_abstime => {
                    self.known[i].identity = sample.identity;
                    self.known[i].seen = true;
                    self.members.push(Member {
                        sample,
                        role: entry.role,
                    });
                    i += 1;
                }
                Ok(_) => {
                    self.issue(entry, "known_pid_reused_not_inherited", None)?;
                    if entry.registered && !entry.seen {
                        self.unpolled_registered += 1;
                    }
                    self.known.swap_remove(i);
                    self.retired += 1;
                }
                Err(e) if absent(entry.identity.pid) => {
                    if entry.registered && !entry.seen {
                        self.issue(entry, "registered_process_never_polled", Some(e))?;
                        self.unpolled_registered += 1;
                    }
                    self.known.swap_remove(i);
                    self.retired += 1;
                }
                Err(e) => {
                    self.issue(entry, "known_identity_unconfirmed", Some(e))?;
                    i += 1;
                }
            }
        }
        // New members are appended and themselves enumerated, but only after the
        // parent's actual Mach birth has been checked before and after enumeration.
        let mut index = 0;
        while index < self.members.len() {
            let parent = self.members[index];
            index += 1;
            match children(parent.sample.identity, origin) {
                Err(kind) => {
                    self.issue(
                        Tracked {
                            identity: parent.sample.identity,
                            role: parent.role,
                            seen: true,
                            registered: false,
                        },
                        kind,
                        None,
                    )?;
                }
                Ok(pids) => {
                    for pid in pids.into_iter().filter(|p| *p != 0) {
                        if self.members.iter().any(|x| x.sample.identity.pid == pid) {
                            continue;
                        }
                        match capture(pid, None, origin) {
                            Ok(sample)
                                if sample.identity.parent_pid == parent.sample.identity.pid =>
                            {
                                let role = self
                                    .known
                                    .iter()
                                    .find(|x| x.identity == sample.identity)
                                    .map_or(Text::new("descendant")?, |x| x.role);
                                self.add(sample.identity, role, false)?;
                                if self.members.len() == CAP {
                                    return Err("native_observer_member_capacity".into());
                                }
                                if let Some(x) = self
                                    .known
                                    .iter_mut()
                                    .find(|x| x.identity == sample.identity)
                                {
                                    x.seen = true;
                                }
                                self.members.push(Member { sample, role });
                            }
                            Ok(sample) => {
                                self.issue(
                                    Tracked {
                                        identity: sample.identity,
                                        role: Text::new("descendant")?,
                                        seen: false,
                                        registered: false,
                                    },
                                    "child_parent_changed_during_discovery",
                                    None,
                                )?;
                            }
                            Err(e) => {
                                if self.issues.len() == CAP {
                                    return Err("native_observer_issue_capacity".into());
                                }
                                self.issues.push(Issue {
                                    pid,
                                    expected_birth: None,
                                    kind: "enumerated_child_unreadable",
                                    read_error: Some(e),
                                });
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
    pub fn live_other_than(&self, pid: u32) -> usize {
        self.known.iter().filter(|x| x.identity.pid != pid).count()
    }
    pub fn owned_identities(&self) -> impl Iterator<Item = Identity> + '_ {
        // Every entry comes from a birth-checked direct spawn/tree edge or the
        // authenticated journal. A child blocked before publishing is still ours.
        self.known.iter().map(|x| x.identity)
    }
}
fn absent(pid: u32) -> bool {
    let Ok(raw) = i32::try_from(pid) else {
        return false;
    };
    let Some(pid) = rustix::process::Pid::from_raw(raw) else {
        return false;
    };
    matches!(
        rustix::process::test_kill_process(pid),
        Err(rustix::io::Errno::SRCH)
    )
}
pub(super) fn children(parent: Identity, origin: Instant) -> Result<Vec<u32>, &'static str> {
    let before = capture(parent.pid, None, origin).map_err(|_| "enumeration_parent_unavailable")?;
    if before.identity.start_abstime != parent.start_abstime {
        return Err("enumeration_parent_reused");
    }
    // libproc 0.14.11 interprets an empty Darwin result via last_os_error,
    // but does not clear that thread-local errno before either syscall. Clear
    // stale unrelated errno via a safe dependency; real syscall errors remain
    // errors. Never turn an actual failed enumeration into an empty success.
    errno::set_errno(errno::Errno(0));
    let pids = libproc::processes::pids_by_type(libproc::processes::ProcFilter::ByParentProcess {
        ppid: parent.pid,
    })
    .map_err(|_| "enumeration_failed")?;
    // libproc allocates from the kernel's initial byte count BEFORE returning.
    // This is a post-return admission, NOT a preallocation guarantee. The fixed
    // observer arena is the outer hard allocation boundary; OOM fails the run.
    if pids.capacity() > 8192 || (!pids.is_empty() && pids.len() == pids.capacity()) {
        return Err("enumeration_capacity_or_possible_truncation");
    }
    let after = capture(parent.pid, None, origin).map_err(|_| "enumeration_parent_lost")?;
    if before.identity != after.identity {
        return Err("enumeration_parent_changed");
    }
    Ok(pids)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn sample(pid: u32) -> Sample {
        Sample {
            identity: Identity {
                pid,
                parent_pid: 1,
                start_abstime: 10,
            },
            begin_ns: 0,
            end_ns: 0,
            usage: crate::native_observer::Usage {
                start_abstime: 10,
                exit_abstime: 0,
                resident_bytes: 1,
                footprint_bytes: 1,
                user_time_raw: 0,
                system_time_raw: 0,
                disk_read_bytes: 0,
                disk_written_bytes: 0,
            },
        }
    }
    #[test]
    fn a_new_parent_cannot_inherit_the_old_birth() {
        let mut t = Tracker::new(sample(1)).unwrap();
        t.add(
            Identity {
                start_abstime: 11,
                ..sample(1).identity
            },
            Text::new("new").unwrap(),
            false,
        )
        .unwrap();
        assert_eq!(t.known.len(), 2);
        assert_eq!(t.identities, 2);
    }
    #[test]
    fn live_enumeration_does_not_inherit_unrelated_errno() {
        let origin = Instant::now();
        let own = capture(std::process::id(), None, origin).unwrap();
        errno::set_errno(errno::Errno(2));
        let result = children(own.identity, origin).unwrap();
        assert!(!result.contains(&std::process::id()));
    }
    #[test]
    fn identity_storage_rejects_before_growing() {
        let mut t = Tracker::new(sample(1)).unwrap();
        for pid in 2..=CAP as u32 {
            t.add(sample(pid).identity, Text::new("x").unwrap(), true)
                .unwrap();
        }
        assert!(t
            .add(sample(999).identity, Text::new("x").unwrap(), true)
            .is_err());
        assert_eq!(t.known.capacity(), CAP);
    }
}
